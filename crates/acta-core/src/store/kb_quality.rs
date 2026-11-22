//! Memory quality in the store (ADR 0015): the verified date, the states
//! (superseded, retired, live again), merge and unmerge, sources, pairs,
//! the local quality log, and the store side of the checker (the
//! full-text prefilter and the store-computable flags).
//!
//! Every state change is an explicit act of the agent or the human, kept
//! as a revision (with the entry's text unchanged) and reversible.

use super::kb::{
    KbUpdate, KbWriteCtx, NewKb, create_in, insert_revision, live_folder, next_rev, read_kb,
    revision_row, update_in,
};
use super::{Store, new_uuid, owner_ids};
use crate::date::{Date, Zone};
use crate::error::{Error, Result};
use crate::kb_quality::{self, Comparison, FlagFacts, KbFlag, Text, Verdict};
use crate::model::{
    Author, ChangeKind, EntryId, FolderId, KbEntry, KbId, KbKind, KbLinkKind, KbLogEvent,
    KbPairKind, KbPairState, KbState, NoteId, Owner, QuestionId, Record, Status, TopicId,
};
use crate::time::{MS_PER_DAY, now_ms};
use rusqlite::{OptionalExtension, params};

/// Most entries one write may supersede (`supersedes`, ADR 0015, 2).
pub const MAX_SUPERSEDES: usize = 3;
/// Longest retire reason, in characters.
pub const RETIRE_REASON_MAX: usize = 200;
/// Longest file path or commit a source may name, in characters.
pub const SOURCE_TEXT_MAX: usize = 300;
/// Candidates the full-text prefilter hands to the checker.
pub const CANDIDATES: usize = 20;
/// How long the local quality log keeps a row.
pub const QUALITY_LOG_DAYS: i64 = 180;

/// A source an entry names (ADR 0015, 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KbSource {
    /// A topic, a timeline entry, a question, a note or another entry.
    Record(Record),
    /// A commit id (7 to 40 hex digits).
    Commit(String),
    /// A path relative to the folder's repository root, with its git blob
    /// id when it was recorded.
    File {
        folder: FolderId,
        path: String,
        blob: Option<String>,
    },
}

/// A stored source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbLink {
    pub id: i64,
    pub kind: KbLinkKind,
    pub target: KbLinkTarget,
    pub folder: Option<FolderId>,
    pub blob: Option<String>,
    pub created_at: i64,
    pub created_by: Author,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KbLinkTarget {
    Record(Record),
    /// A record this database does not have (an import without it).
    Missing,
    Text(String),
}

/// The write a supersede goes with: a new entry, or an edit of the
/// survivor.
#[derive(Clone, Debug)]
pub enum KbSuccessor {
    Create(NewKb),
    Update(KbId, KbUpdate),
}

/// A found pair.
#[derive(Clone, Debug, PartialEq)]
pub struct KbPair {
    pub a: KbId,
    pub b: KbId,
    pub kind: KbPairKind,
    pub state: KbPairState,
    pub score: f64,
    pub created_at: i64,
    pub created_by: Author,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<Author>,
}

impl KbPair {
    /// The other side of the pair.
    pub fn other(&self, id: KbId) -> KbId {
        if self.a == id { self.b } else { self.a }
    }
}

/// What a merge changed, so the app's undo can take it back in one step
/// (the contacts pattern).
#[derive(Clone, Debug, PartialEq)]
pub struct KbMergeTrace {
    pub survivor: KbId,
    pub other: KbId,
    /// The survivor's revision before the merge, and the one it made.
    pub survivor_rev: i64,
    pub merged_rev: i64,
    pub owner_before: Owner,
    pub verified_before: (i64, Author),
    /// Sources copied from the other entry.
    pub copied_links: Vec<i64>,
    /// The pairs between the two before the merge.
    pub pairs_before: Vec<KbPair>,
}

/// One row of the local quality log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbLogRow {
    pub at: i64,
    pub event: KbLogEvent,
    pub entry: Option<KbId>,
    pub other: Option<KbId>,
    pub session: Option<String>,
    pub author: Author,
}

/// A live memory the prefilter found in the shared scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbCandidate {
    pub id: KbId,
    pub rev: i64,
    pub owner: Owner,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub verified_at: i64,
}

/// A candidate the checker kept, best first.
#[derive(Clone, Debug, PartialEq)]
pub struct KbFinding {
    pub candidate: KbCandidate,
    pub comparison: Comparison,
}

impl Store {
    /// Writes `successor` and supersedes `olds` with it, in one
    /// transaction (ADR 0015, 2). Returns the successor and its revision.
    pub fn kb_supersede(
        &mut self,
        successor: KbSuccessor,
        olds: &[KbId],
        ctx: &KbWriteCtx,
        author: Author,
    ) -> Result<(KbId, i64)> {
        let tx = self.write()?;
        let now = now_ms();
        let (id, rev) = match successor {
            KbSuccessor::Create(new) => (create_in(&tx, new, ctx, author, now)?, 1),
            KbSuccessor::Update(id, update) => (id, update_in(&tx, id, update, ctx, author, now)?),
        };
        supersede_in(&tx, id, olds, ctx, author, now)?;
        tx.commit()?;
        Ok((id, rev))
    }

    /// Vouches that an entry is still true (ADR 0015, 1): no new text, a
    /// revision with change `verified`, and the blob ids of its file
    /// sources recorded again (`blobs`: link id and blob). A stale
    /// `expected_rev` is refused: you cannot vouch for a text you did not
    /// read.
    pub fn kb_verify(
        &mut self,
        id: KbId,
        expected_rev: Option<i64>,
        blobs: &[(i64, Option<String>)],
        ctx: &KbWriteCtx,
        author: Author,
    ) -> Result<i64> {
        let tx = self.write()?;
        let cur = checked(&tx, id, expected_rev)?;
        live(&cur)?;
        let now = now_ms();
        let rev = state_revision(&tx, &cur, ChangeKind::Verified, None, ctx, author, now)?;
        tx.execute(
            "UPDATE kb_entries SET verified_at = ?2, verified_by = ?3 WHERE id = ?1",
            params![id.0, now, author],
        )?;
        for (link, blob) in blobs {
            tx.execute(
                "UPDATE kb_links SET blob = ?3 WHERE id = ?1 AND entry_id = ?2",
                params![link, id.0, blob],
            )?;
        }
        tx.commit()?;
        Ok(rev)
    }

    /// Retires an entry: no longer true, no successor. The reason (at most
    /// [`RETIRE_REASON_MAX`] chars, may be empty) is the revision's note.
    /// Not a delete: it stays readable by id and can be made live again.
    pub fn kb_retire(
        &mut self,
        id: KbId,
        expected_rev: Option<i64>,
        reason: &str,
        ctx: &KbWriteCtx,
        author: Author,
    ) -> Result<i64> {
        let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
        let n = reason.chars().count();
        if n > RETIRE_REASON_MAX {
            return Err(Error::Invalid(format!(
                "the reason is {n} chars; the limit is {RETIRE_REASON_MAX}"
            )));
        }
        let tx = self.write()?;
        let cur = checked(&tx, id, expected_rev)?;
        live(&cur)?;
        let now = now_ms();
        let note = (!reason.is_empty()).then_some(reason.as_str());
        let rev = state_revision(&tx, &cur, ChangeKind::Retired, note, ctx, author, now)?;
        tx.execute(
            "UPDATE kb_entries SET state = 'retired', superseded_by = NULL WHERE id = ?1",
            [id.0],
        )?;
        tx.commit()?;
        Ok(rev)
    }

    /// Makes a superseded or retired entry live again ("Make live again",
    /// and unmerge after the undo window). Its pairs stay as they are.
    pub fn kb_make_live(&mut self, id: KbId, author: Author) -> Result<i64> {
        let tx = self.write()?;
        let cur = read_kb(&tx, id)?;
        if cur.state == KbState::Live {
            return Err(Error::Invalid(format!("{id} is already live")));
        }
        let rev = make_live_in(&tx, &cur, author, now_ms())?;
        tx.commit()?;
        Ok(rev)
    }

    /// Merges `other` into `survivor` (ADR 0015, 3): the survivor gets a
    /// revision whose body is its text, a blank line and the other's body,
    /// lives at the narrowest level that holds both, and gets copies of
    /// the other's sources; the other is superseded by it; their pairs
    /// are resolved. One transaction; [`Store::kb_unmerge`] takes it back.
    pub fn kb_merge(
        &mut self,
        survivor: KbId,
        other: KbId,
        author: Author,
    ) -> Result<KbMergeTrace> {
        if survivor == other {
            return Err(Error::Invalid(
                "an entry cannot be merged into itself".into(),
            ));
        }
        let tx = self.write()?;
        let s = read_kb(&tx, survivor)?;
        let o = read_kb(&tx, other)?;
        live(&s)?;
        live(&o)?;
        if s.kind != o.kind {
            return Err(Error::Invalid(format!(
                "{other} is a {} and {survivor} a {}",
                o.kind.as_str(),
                s.kind.as_str()
            )));
        }
        if s.project != o.project {
            return Err(Error::Invalid(format!(
                "{other} is in another project than {survivor}"
            )));
        }
        let now = now_ms();
        let owner = owner_join(&tx, s.owner, o.owner)?;
        let pairs_before = pairs_between(&tx, survivor, other)?;
        let body = match (s.body.trim().is_empty(), o.body.trim().is_empty()) {
            (_, true) => s.body.clone(),
            (true, false) => o.body.clone(),
            (false, false) => format!("{}\n\n{}", s.body.trim_end(), o.body.trim_start()),
        };
        let (project, epic, topic) = owner_ids(&tx, owner)?;
        let rev = next_rev(&tx, survivor)?;
        tx.execute(
            "UPDATE kb_entries SET project_id = ?2, epic_id = ?3, topic_id = ?4, body = ?5, rev = ?6,
                    updated_at = ?7, updated_by = ?8, verified_at = ?7, verified_by = ?8
             WHERE id = ?1",
            params![survivor.0, project, epic, topic, body, rev, now, author],
        )?;
        insert_revision(
            &tx,
            survivor,
            rev,
            &s.title,
            &s.summary,
            &body,
            author,
            ChangeKind::Merged,
            None,
            now,
            &KbWriteCtx::default(),
        )?;
        let mut copied_links = Vec::new();
        for link in links_of(&tx, other)? {
            if let Some(id) = copy_link(&tx, link, survivor, author, now)? {
                copied_links.push(id);
            }
        }
        supersede_one(&tx, survivor, &o, &KbWriteCtx::default(), author, now)?;
        tx.commit()?;
        Ok(KbMergeTrace {
            survivor,
            other,
            survivor_rev: s.rev,
            merged_rev: rev,
            owner_before: s.owner,
            verified_before: (s.verified_at, s.verified_by),
            copied_links,
            pairs_before,
        })
    }

    /// Takes back a merge (the app's undo): the survivor's text, level and
    /// verified date as they were (a new revision), the copied sources
    /// removed, the other entry live, the pairs as they were. Refused when
    /// the survivor changed since the merge.
    pub fn kb_unmerge(&mut self, trace: &KbMergeTrace, author: Author) -> Result<()> {
        let tx = self.write()?;
        let s = read_kb(&tx, trace.survivor)?;
        if s.rev != trace.merged_rev {
            return Err(Error::Invalid(format!(
                "{} changed since the merge",
                trace.survivor
            )));
        }
        let o = read_kb(&tx, trace.other)?;
        if o.superseded_by != Some(trace.survivor) {
            return Err(Error::Invalid(format!(
                "{} is no longer merged into {}",
                trace.other, trace.survivor
            )));
        }
        let now = now_ms();
        let old = tx
            .query_row(
                "SELECT uuid, rev, title, summary, body, author, change, section, created_at
                 FROM kb_revisions WHERE entry_id = ?1 AND rev = ?2",
                params![trace.survivor.0, trace.survivor_rev],
                revision_row,
            )
            .optional()?
            .ok_or_else(|| {
                Error::NotFound(format!("{} rev {}", trace.survivor, trace.survivor_rev))
            })?;
        let (project, epic, topic) = owner_ids(&tx, trace.owner_before)?;
        let rev = next_rev(&tx, trace.survivor)?;
        tx.execute(
            "UPDATE kb_entries SET project_id = ?2, epic_id = ?3, topic_id = ?4, title = ?5, summary = ?6,
                    body = ?7, rev = ?8, updated_at = ?9, updated_by = ?10, verified_at = ?11, verified_by = ?12
             WHERE id = ?1",
            params![
                trace.survivor.0,
                project,
                epic,
                topic,
                old.title,
                old.summary,
                old.body,
                rev,
                now,
                author,
                trace.verified_before.0,
                trace.verified_before.1
            ],
        )?;
        insert_revision(
            &tx,
            trace.survivor,
            rev,
            &old.title,
            &old.summary,
            &old.body,
            author,
            ChangeKind::Restored,
            Some(&format!("rev {}", trace.survivor_rev)),
            now,
            &KbWriteCtx::default(),
        )?;
        for link in &trace.copied_links {
            tx.execute(
                "DELETE FROM kb_links WHERE id = ?1 AND entry_id = ?2",
                params![link, trace.survivor.0],
            )?;
        }
        make_live_in(&tx, &o, author, now)?;
        let (a, b) = ordered(trace.survivor, trace.other);
        tx.execute(
            "DELETE FROM kb_pairs WHERE a = ?1 AND b = ?2",
            params![a.0, b.0],
        )?;
        for p in &trace.pairs_before {
            tx.execute(
                "INSERT INTO kb_pairs (a, b, kind, state, score, created_at, created_by, resolved_at, resolved_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![p.a.0, p.b.0, p.kind, p.state, p.score, p.created_at, p.created_by,
                        p.resolved_at, p.resolved_by],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Adds sources to an entry (duplicates of one it has are skipped).
    /// Returns the new link ids.
    pub fn kb_add_links(
        &mut self,
        id: KbId,
        sources: &[KbSource],
        author: Author,
    ) -> Result<Vec<i64>> {
        let tx = self.write()?;
        read_kb(&tx, id)?;
        let ids = add_links(&tx, id, sources, author, now_ms())?;
        tx.commit()?;
        Ok(ids)
    }

    /// An entry's sources, oldest first.
    pub fn kb_links(&self, id: KbId) -> Result<Vec<KbLink>> {
        read_kb(&self.conn, id)?;
        links_of(&self.conn, id)
    }

    /// Records a pair the checker found, as open unless it is known.
    /// Returns its state (a pair kept apart stays `distinct`).
    pub fn kb_record_pair(
        &mut self,
        x: KbId,
        y: KbId,
        kind: KbPairKind,
        score: f64,
        author: Author,
    ) -> Result<KbPairState> {
        let (a, b) = pair_ids(x, y)?;
        let tx = self.write()?;
        read_kb(&tx, a)?;
        read_kb(&tx, b)?;
        tx.execute(
            "INSERT OR IGNORE INTO kb_pairs (a, b, kind, state, score, created_at, created_by)
             VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?6)",
            params![a.0, b.0, kind, score, now_ms(), author],
        )?;
        let state = tx.query_row(
            "SELECT state FROM kb_pairs WHERE a = ?1 AND b = ?2 AND kind = ?3",
            params![a.0, b.0, kind],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(state)
    }

    /// Sets a pair's state: `distinct` ("keep both"), `resolved`, or `open`
    /// again. A pair not found yet is recorded with a score of 0.
    pub fn kb_set_pair_state(
        &mut self,
        x: KbId,
        y: KbId,
        kind: KbPairKind,
        state: KbPairState,
        author: Author,
    ) -> Result<()> {
        let (a, b) = pair_ids(x, y)?;
        let tx = self.write()?;
        read_kb(&tx, a)?;
        read_kb(&tx, b)?;
        let now = now_ms();
        let (at, by) = match state {
            KbPairState::Open => (None, None),
            _ => (Some(now), Some(author)),
        };
        tx.execute(
            "INSERT INTO kb_pairs (a, b, kind, state, score, created_at, created_by, resolved_at, resolved_by)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8)
             ON CONFLICT (a, b, kind) DO UPDATE SET state = ?4, resolved_at = ?7, resolved_by = ?8",
            params![a.0, b.0, kind, state, now, author, at, by],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The Claude Code session that wrote a revision, if one did.
    pub fn kb_revision_session(&self, id: KbId, rev: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT session FROM kb_revisions WHERE entry_id = ?1 AND rev = ?2",
                params![id.0, rev],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Every pair an entry is in.
    pub fn kb_pairs_of(&self, id: KbId) -> Result<Vec<KbPair>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {PAIR_COLS} FROM kb_pairs WHERE a = ?1 OR b = ?1 ORDER BY a, b, kind"
        ))?;
        let rows = stmt.query_map([id.0], pair_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Records a quality event (ADR 0015, 8) and prunes rows older than
    /// [`QUALITY_LOG_DAYS`]. Local: never exported.
    pub fn kb_log(
        &mut self,
        event: KbLogEvent,
        entry: Option<KbId>,
        other: Option<KbId>,
        session: Option<&str>,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO kb_quality_log (at, event, entry_id, other_id, session, author)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                now,
                event,
                entry.map(|k| k.0),
                other.map(|k| k.0),
                session,
                author
            ],
        )?;
        tx.execute(
            "DELETE FROM kb_quality_log WHERE at < ?1",
            [now - QUALITY_LOG_DAYS * MS_PER_DAY],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The quality log since an instant, oldest first.
    pub fn kb_log_since(&self, since: i64) -> Result<Vec<KbLogRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, event, entry_id, other_id, session, author FROM kb_quality_log
             WHERE at >= ?1 ORDER BY at, id",
        )?;
        let rows = stmt.query_map([since], |r| {
            Ok(KbLogRow {
                at: r.get(0)?,
                event: r.get(1)?,
                entry: r.get::<_, Option<i64>>(2)?.map(KbId),
                other: r.get::<_, Option<i64>>(3)?.map(KbId),
                session: r.get(4)?,
                author: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The live memories a memory at `owner` shares a scope with (its
    /// owner, ancestors and descendants) that one full-text query over the
    /// draft's content words finds, best [`CANDIDATES`] by bm25.
    /// `exclude` leaves out the entry being edited.
    pub fn kb_candidates(
        &self,
        owner: Owner,
        draft: &Text<'_>,
        exclude: Option<KbId>,
    ) -> Result<Vec<KbCandidate>> {
        let Some(fts) = kb_quality::fts_terms(draft) else {
            return Ok(Vec::new());
        };
        let up = self.scope_condition(owner)?;
        let down = match owner {
            Owner::Project(p) => format!("k.project_id = {}", p.0),
            Owner::Epic(e) => format!(
                "k.topic_id IN (SELECT id FROM topics WHERE epic_id = {} AND deleted_at IS NULL)",
                e.0
            ),
            Owner::Topic(_) => "0".into(),
        };
        let exclude = exclude.map_or(String::new(), |k| format!(" AND k.id <> {}", k.0));
        let mut stmt = self.conn.prepare(&format!(
            "SELECT k.id, k.rev, k.project_id, k.epic_id, k.topic_id, k.title, k.summary, k.body,
                    coalesce(k.verified_at, k.updated_at)
             FROM kb_fts JOIN kb_entries k ON k.id = kb_fts.rowid
             WHERE kb_fts MATCH ?1 AND k.deleted_at IS NULL AND k.state = 'live' AND k.kind = 'memory'
               AND ({up} OR {down}){exclude}
             ORDER BY bm25(kb_fts), k.id LIMIT {CANDIDATES}"
        ))?;
        let rows = stmt.query_map([&fts], |r| {
            Ok(KbCandidate {
                id: KbId(r.get(0)?),
                rev: r.get(1)?,
                owner: super::owner_at(r, 2)?,
                title: r.get(5)?,
                summary: r.get(6)?,
                body: r.get(7)?,
                verified_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The checker at write time (ADR 0015, 2 and 3): the candidates of
    /// [`Store::kb_candidates`] scored against the draft, those found
    /// related or worse, best first (same, conflict, duplicate, related;
    /// higher score first). A pair kept apart (`distinct`) of that kind
    /// is left out.
    pub fn kb_check(
        &self,
        owner: Owner,
        draft: &Text<'_>,
        exclude: Option<KbId>,
    ) -> Result<Vec<KbFinding>> {
        let mine = kb_quality::profile(draft);
        let mut out = Vec::new();
        for c in self.kb_candidates(owner, draft, exclude)? {
            let theirs = kb_quality::profile(&Text {
                title: &c.title,
                summary: &c.summary,
                body: &c.body,
            });
            let comparison = kb_quality::compare(&mine, &theirs);
            let kind = match comparison.verdict {
                Verdict::Unrelated => continue,
                Verdict::Conflict => Some(KbPairKind::Conflict),
                Verdict::Same | Verdict::Duplicate => Some(KbPairKind::Duplicate),
                Verdict::Related => None,
            };
            if let (Some(kind), Some(me)) = (kind, exclude)
                && self.pair_state(me, c.id, kind)? == Some(KbPairState::Distinct)
            {
                continue;
            }
            out.push(KbFinding {
                candidate: c,
                comparison,
            });
        }
        out.sort_by(|x, y| {
            x.comparison
                .verdict
                .cmp(&y.comparison.verdict)
                .then(y.comparison.s.total_cmp(&x.comparison.s))
                .then(x.candidate.id.0.cmp(&y.candidate.id.0))
        });
        Ok(out)
    }

    /// The store-computable flags of some entries (ADR 0015, 1), in the
    /// order given; an id not found has none.
    pub fn kb_store_flags(
        &self,
        ids: &[KbId],
        zone: Zone,
        today: Date,
    ) -> Result<Vec<(KbId, Vec<KbFlag>)>> {
        let mut out = Vec::with_capacity(ids.len());
        let mut pairs = self.conn.prepare(
            "SELECT p.kind, CASE WHEN p.a = ?1 THEN p.b ELSE p.a END AS o
             FROM kb_pairs p JOIN kb_entries k ON k.id = CASE WHEN p.a = ?1 THEN p.b ELSE p.a END
             WHERE (p.a = ?1 OR p.b = ?1) AND p.state = 'open'
               AND k.deleted_at IS NULL AND k.state = 'live'",
        )?;
        for &id in ids {
            let facts = self
                .conn
                .query_row(
                    "SELECT k.kind, coalesce(k.verified_at, k.updated_at), t.status,
                            k.context_folder_id IS NOT NULL
                                AND (f.id IS NULL OR f.deleted_at IS NOT NULL)
                     FROM kb_entries k
                     LEFT JOIN topics t ON t.id = k.topic_id
                     LEFT JOIN folders f ON f.id = k.context_folder_id
                     WHERE k.id = ?1 AND k.deleted_at IS NULL",
                    [id.0],
                    |r| {
                        Ok((
                            r.get::<_, KbKind>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, Option<Status>>(2)?,
                            r.get::<_, bool>(3)?,
                        ))
                    },
                )
                .optional()?;
            let Some((kind, verified, topic_status, folder_removed)) = facts else {
                out.push((id, Vec::new()));
                continue;
            };
            let found = pairs
                .query_map([id.0], |r| {
                    Ok((r.get::<_, KbPairKind>(0)?, KbId(r.get(1)?)))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let facts = FlagFacts {
                kind,
                verified_on: zone.date_of(verified),
                topic_status,
                pairs: found,
                folder_removed,
            };
            out.push((id, kb_quality::kb_flags(&facts, today)));
        }
        Ok(out)
    }

    fn pair_state(&self, x: KbId, y: KbId, kind: KbPairKind) -> Result<Option<KbPairState>> {
        let (a, b) = ordered(x, y);
        Ok(self
            .conn
            .query_row(
                "SELECT state FROM kb_pairs WHERE a = ?1 AND b = ?2 AND kind = ?3",
                params![a.0, b.0, kind],
                |r| r.get(0),
            )
            .optional()?)
    }
}

/// The figures behind `acta kb quality` (ADR 0015, 8) for one project
/// since an instant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KbQualityReport {
    pub live_memories: i64,
    pub unflagged: i64,
    /// Store flags: old, topic closed, conflict, duplicate, folder removed.
    pub flags: [i64; 5],
    pub claude_writes: i64,
    pub human_corrections: i64,
    /// Supersedes and merges by the human, and those of pairs the checker
    /// never found.
    pub human_merges: i64,
    pub missed: i64,
    pub warnings: i64,
    pub kept_both: i64,
    pub verifies: i64,
    pub sessions: i64,
    pub flagged_reads: i64,
    pub flagged_acted: i64,
    pub stale_refused: i64,
    pub auto_merged: i64,
    pub stale_recovered: i64,
}

impl Store {
    /// Computes [`KbQualityReport`] from the entries, their revisions and
    /// the local log. Nothing leaves the machine.
    pub fn kb_quality_report(
        &self,
        project: crate::model::ProjectId,
        since: i64,
        zone: Zone,
        today: Date,
    ) -> Result<KbQualityReport> {
        let p = project.0;
        let mut r = KbQualityReport::default();
        let ids: Vec<KbId> = {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM kb_entries WHERE project_id = ?1 AND kind = 'memory'
                 AND deleted_at IS NULL AND state = 'live'",
            )?;
            stmt.query_map([p], |r| Ok(KbId(r.get(0)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        r.live_memories = ids.len() as i64;
        for (_, flags) in self.kb_store_flags(&ids, zone, today)? {
            if flags.is_empty() {
                r.unflagged += 1;
            }
            for f in flags {
                let i = match f {
                    KbFlag::Old { .. } => 0,
                    KbFlag::TopicClosed(_) => 1,
                    KbFlag::Conflict(_) => 2,
                    KbFlag::Duplicate(_) => 3,
                    KbFlag::FolderRemoved => 4,
                };
                r.flags[i] += 1;
            }
        }
        let count = |sql: &str| -> Result<i64> {
            Ok(self.conn.query_row(sql, params![p, since], |x| x.get(0))?)
        };
        const MEM: &str = "JOIN kb_entries k ON k.id = r.entry_id WHERE k.project_id = ?1 AND k.kind = 'memory' AND r.created_at >= ?2";
        r.claude_writes = count(&format!(
            "SELECT count(*) FROM kb_revisions r {MEM} AND r.author = 'claude'
             AND r.change IN ('created', 'created_from_file', 'edited', 'replaced_section', 'appended_section')"
        ))?;
        r.human_corrections = count(&format!(
            "SELECT count(*) FROM kb_revisions r {MEM} AND r.author = 'human' AND k.created_by = 'claude'
             AND r.change IN ('edited', 'replaced_section', 'appended_section', 'retired', 'superseded', 'moved')"
        ))? + count(
            "SELECT count(*) FROM kb_entries WHERE project_id = ?1 AND kind = 'memory'
             AND created_by = 'claude' AND deleted_by = 'human' AND deleted_at >= ?2",
        )?;
        r.human_merges = count(&format!(
            "SELECT count(*) FROM kb_revisions r {MEM} AND r.author = 'human' AND r.change = 'superseded'"
        ))?;
        r.missed = count(&format!(
            "SELECT count(*) FROM kb_revisions r {MEM} AND r.author = 'human' AND r.change = 'superseded'
             AND k.superseded_by IS NOT NULL AND NOT EXISTS (SELECT 1 FROM kb_pairs x
                 WHERE x.a = min(k.id, k.superseded_by) AND x.b = max(k.id, k.superseded_by))"
        ))?;
        let in_project = "(l.entry_id IN (SELECT id FROM kb_entries WHERE project_id = ?1)
             OR l.other_id IN (SELECT id FROM kb_entries WHERE project_id = ?1)) AND l.at >= ?2";
        let event = |e: &str| -> Result<i64> {
            count(&format!(
                "SELECT count(*) FROM kb_quality_log l WHERE l.event = '{e}' AND {in_project}"
            ))
        };
        r.warnings = event("duplicate_warning")? + event("conflict_warning")?;
        r.kept_both = event("kept_both")?;
        r.flagged_reads = event("flagged_read")?;
        r.stale_refused = event("stale_refused")?;
        r.auto_merged = event("auto_merged")?;
        r.verifies = count(&format!(
            "SELECT count(*) FROM kb_revisions r {MEM} AND r.author = 'claude' AND r.change = 'verified'"
        ))?;
        r.sessions = count(&format!(
            "SELECT count(DISTINCT r.session) FROM kb_revisions r {MEM} AND r.author = 'claude'
             AND r.session IS NOT NULL"
        ))?;
        r.flagged_acted = count(&format!(
            "SELECT count(*) FROM kb_quality_log l WHERE l.event = 'flagged_read' AND {in_project}
             AND EXISTS (SELECT 1 FROM kb_revisions v WHERE v.entry_id = l.entry_id
                 AND v.session = l.session AND v.created_at >= l.at
                 AND v.change IN ('verified', 'edited', 'replaced_section', 'appended_section', 'retired', 'superseded'))"
        ))?;
        r.stale_recovered = count(&format!(
            "SELECT count(*) FROM kb_quality_log l WHERE l.event = 'stale_refused' AND {in_project}
             AND EXISTS (SELECT 1 FROM kb_revisions v WHERE v.entry_id = l.entry_id
                 AND v.session = l.session AND v.created_at >= l.at AND v.created_at < l.at + 600000)"
        ))?;
        Ok(r)
    }
}

const PAIR_COLS: &str =
    "a, b, kind, state, score, created_at, created_by, resolved_at, resolved_by";

fn pair_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<KbPair> {
    Ok(KbPair {
        a: KbId(r.get(0)?),
        b: KbId(r.get(1)?),
        kind: r.get(2)?,
        state: r.get(3)?,
        score: r.get(4)?,
        created_at: r.get(5)?,
        created_by: r.get(6)?,
        resolved_at: r.get(7)?,
        resolved_by: r.get(8)?,
    })
}

fn ordered(x: KbId, y: KbId) -> (KbId, KbId) {
    if x.0 < y.0 { (x, y) } else { (y, x) }
}

fn pair_ids(x: KbId, y: KbId) -> Result<(KbId, KbId)> {
    if x == y {
        return Err(Error::Invalid("a pair needs two entries".into()));
    }
    Ok(ordered(x, y))
}

fn pairs_between(tx: &rusqlite::Connection, x: KbId, y: KbId) -> Result<Vec<KbPair>> {
    let (a, b) = ordered(x, y);
    let mut stmt = tx.prepare(&format!(
        "SELECT {PAIR_COLS} FROM kb_pairs WHERE a = ?1 AND b = ?2 ORDER BY kind"
    ))?;
    let rows = stmt.query_map(params![a.0, b.0], pair_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Reads an entry for a state change and checks the writer's revision.
fn checked(tx: &rusqlite::Connection, id: KbId, expected_rev: Option<i64>) -> Result<KbEntry> {
    let cur = read_kb(tx, id)?;
    if let Some(expected) = expected_rev
        && expected != cur.rev
    {
        return Err(Error::StaleRevision { current: cur.rev });
    }
    Ok(cur)
}

fn live(e: &KbEntry) -> Result<()> {
    match e.state {
        KbState::Live => Ok(()),
        s => Err(Error::Invalid(format!(
            "{} is {}; make it live first",
            e.id,
            s.as_str()
        ))),
    }
}

/// A revision for a change that keeps the entry's text: the next number,
/// the entry's current title, summary and body.
fn state_revision(
    tx: &rusqlite::Connection,
    cur: &KbEntry,
    change: ChangeKind,
    note: Option<&str>,
    ctx: &KbWriteCtx,
    author: Author,
    now: i64,
) -> Result<i64> {
    let rev = next_rev(tx, cur.id)?;
    tx.execute(
        "UPDATE kb_entries SET rev = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
        params![cur.id.0, rev, now, author],
    )?;
    insert_revision(
        tx,
        cur.id,
        rev,
        &cur.title,
        &cur.summary,
        &cur.body,
        author,
        change,
        note,
        now,
        ctx,
    )?;
    Ok(rev)
}

fn make_live_in(tx: &rusqlite::Connection, cur: &KbEntry, author: Author, now: i64) -> Result<i64> {
    let rev = state_revision(
        tx,
        cur,
        ChangeKind::MadeLive,
        None,
        &KbWriteCtx::default(),
        author,
        now,
    )?;
    tx.execute(
        "UPDATE kb_entries SET state = 'live', superseded_by = NULL WHERE id = ?1",
        [cur.id.0],
    )?;
    Ok(rev)
}

/// Supersedes each of `olds` with `by` (checked: one to three, distinct,
/// live, in the same project, not `by` itself).
fn supersede_in(
    tx: &rusqlite::Connection,
    by: KbId,
    olds: &[KbId],
    ctx: &KbWriteCtx,
    author: Author,
    now: i64,
) -> Result<()> {
    if olds.is_empty() || olds.len() > MAX_SUPERSEDES {
        return Err(Error::Invalid(format!(
            "supersedes names 1 to {MAX_SUPERSEDES} entries"
        )));
    }
    let successor = read_kb(tx, by)?;
    for (i, &old) in olds.iter().enumerate() {
        if old == by {
            return Err(Error::Invalid(format!("{old} cannot supersede itself")));
        }
        if olds[..i].contains(&old) {
            return Err(Error::Invalid(format!("{old} is named twice")));
        }
        let cur = read_kb(tx, old)?;
        if cur.state != KbState::Live {
            return Err(Error::Invalid(format!(
                "{old} is already {}",
                cur.state.as_str()
            )));
        }
        if cur.project != successor.project {
            return Err(Error::Invalid(format!("{old} is in another project")));
        }
        supersede_one(tx, by, &cur, ctx, author, now)?;
    }
    Ok(())
}

fn supersede_one(
    tx: &rusqlite::Connection,
    by: KbId,
    old: &KbEntry,
    ctx: &KbWriteCtx,
    author: Author,
    now: i64,
) -> Result<()> {
    state_revision(tx, old, ChangeKind::Superseded, None, ctx, author, now)?;
    tx.execute(
        "UPDATE kb_entries SET state = 'superseded', superseded_by = ?2 WHERE id = ?1",
        params![old.id.0, by.0],
    )?;
    let (a, b) = ordered(by, old.id);
    tx.execute(
        "UPDATE kb_pairs SET state = 'resolved', resolved_at = ?3, resolved_by = ?4
         WHERE a = ?1 AND b = ?2 AND state <> 'resolved'",
        params![a.0, b.0, now, author],
    )?;
    Ok(())
}

/// The narrowest owner that holds both: the same owner, else the common
/// epic, else the project.
fn owner_join(tx: &rusqlite::Connection, x: Owner, y: Owner) -> Result<Owner> {
    if x == y {
        return Ok(x);
    }
    let chain = |o: Owner| -> Result<(crate::model::ProjectId, Option<crate::model::EpicId>)> {
        Ok(match o {
            Owner::Project(p) => (p, None),
            Owner::Epic(e) => (super::records::read_epic(tx, e)?.project, Some(e)),
            Owner::Topic(t) => {
                let t = super::records::read_topic(tx, t)?;
                (t.project, t.epic)
            }
        })
    };
    let (px, ex) = chain(x)?;
    let (py, ey) = chain(y)?;
    if px != py {
        return Err(Error::Invalid(
            "the entries are in different projects".into(),
        ));
    }
    Ok(match (ex, ey) {
        (Some(a), Some(b)) if a == b => Owner::Epic(a),
        _ => Owner::Project(px),
    })
}

/// The table and link kind of a record a source may name.
fn record_target(r: Record) -> Result<(KbLinkKind, &'static str, i64)> {
    Ok(match r {
        Record::Topic(TopicId(id)) => (KbLinkKind::Topic, "topics", id),
        Record::Entry(EntryId(id)) => (KbLinkKind::Entry, "entries", id),
        Record::Question(QuestionId(id)) => (KbLinkKind::Question, "questions", id),
        Record::Note(NoteId(id)) => (KbLinkKind::Note, "notes", id),
        Record::Kb(KbId(id)) => (KbLinkKind::Kb, "kb_entries", id),
        other => {
            return Err(Error::Invalid(format!(
                "{other} cannot be a source (a topic, entry, question, note or K id can)"
            )));
        }
    })
}

fn table_of(kind: KbLinkKind) -> Option<&'static str> {
    Some(match kind {
        KbLinkKind::Topic => "topics",
        KbLinkKind::Entry => "entries",
        KbLinkKind::Question => "questions",
        KbLinkKind::Note => "notes",
        KbLinkKind::Kb => "kb_entries",
        KbLinkKind::Commit | KbLinkKind::File => return None,
    })
}

fn to_record(kind: KbLinkKind, id: i64) -> Option<Record> {
    Some(match kind {
        KbLinkKind::Topic => Record::Topic(TopicId(id)),
        KbLinkKind::Entry => Record::Entry(EntryId(id)),
        KbLinkKind::Question => Record::Question(QuestionId(id)),
        KbLinkKind::Note => Record::Note(NoteId(id)),
        KbLinkKind::Kb => Record::Kb(KbId(id)),
        KbLinkKind::Commit | KbLinkKind::File => return None,
    })
}

fn is_commit(text: &str) -> bool {
    (7..=40).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Adds `sources` to entry `id` inside the caller's transaction; one it
/// already names is skipped. When every file source sits in one folder
/// and the entry has no context folder, that folder becomes it (ADR
/// 0015, 4).
pub(crate) fn add_links(
    tx: &rusqlite::Connection,
    id: KbId,
    sources: &[KbSource],
    author: Author,
    now: i64,
) -> Result<Vec<i64>> {
    let mut out = Vec::new();
    for src in sources {
        let (kind, target_uuid, target_text, folder, blob) = match src {
            KbSource::Record(r) => {
                let (kind, table, rid) = record_target(*r)?;
                let uuid: String = tx
                    .query_row(
                        &format!("SELECT uuid FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                        [rid],
                        |row| row.get(0),
                    )
                    .optional()?
                    .ok_or_else(|| Error::NotFound(r.to_string()))?;
                (kind, Some(uuid), None, None, None)
            }
            KbSource::Commit(c) => {
                let c = c.trim().to_ascii_lowercase();
                if !is_commit(&c) {
                    return Err(Error::Invalid("a commit is 7 to 40 hex digits".into()));
                }
                (KbLinkKind::Commit, None, Some(c), None, None)
            }
            KbSource::File { folder, path, blob } => {
                let path = path.trim().replace('\\', "/");
                let n = path.chars().count();
                if path.is_empty() || n > SOURCE_TEXT_MAX {
                    return Err(Error::Invalid(format!(
                        "a file path is 1 to {SOURCE_TEXT_MAX} chars"
                    )));
                }
                if path.starts_with('/') || path.contains(':') || path.split('/').any(|s| s == "..")
                {
                    return Err(Error::Invalid(
                        "a file path is relative to the repository root".into(),
                    ));
                }
                live_folder(tx, *folder)?;
                (
                    KbLinkKind::File,
                    None,
                    Some(path),
                    Some(folder.0),
                    blob.clone(),
                )
            }
        };
        let known: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM kb_links WHERE entry_id = ?1 AND kind = ?2
                 AND target_uuid IS ?3 AND target_text IS ?4 AND folder_id IS ?5)",
            params![id.0, kind, target_uuid, target_text, folder],
            |r| r.get(0),
        )?;
        if known {
            continue;
        }
        tx.execute(
            "INSERT INTO kb_links (uuid, entry_id, kind, target_uuid, target_text, folder_id, blob, created_at, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![new_uuid(), id.0, kind, target_uuid, target_text, folder, blob, now, author],
        )?;
        out.push(tx.last_insert_rowid());
    }
    if !out.is_empty() {
        tx.execute(
            "UPDATE kb_entries SET context_folder_id = (
                 SELECT min(folder_id) FROM kb_links WHERE entry_id = ?1 AND kind = 'file')
             WHERE id = ?1 AND context_folder_id IS NULL
               AND (SELECT count(DISTINCT folder_id) FROM kb_links WHERE entry_id = ?1 AND kind = 'file') = 1",
            [id.0],
        )?;
    }
    Ok(out)
}

struct RawLink {
    id: i64,
    kind: KbLinkKind,
    target_uuid: Option<String>,
    target_text: Option<String>,
    folder: Option<i64>,
    blob: Option<String>,
    created_at: i64,
    created_by: Author,
}

fn raw_links(tx: &rusqlite::Connection, id: KbId) -> Result<Vec<RawLink>> {
    let mut stmt = tx.prepare(
        "SELECT id, kind, target_uuid, target_text, folder_id, blob, created_at, created_by
         FROM kb_links WHERE entry_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([id.0], |r| {
        Ok(RawLink {
            id: r.get(0)?,
            kind: r.get(1)?,
            target_uuid: r.get(2)?,
            target_text: r.get(3)?,
            folder: r.get(4)?,
            blob: r.get(5)?,
            created_at: r.get(6)?,
            created_by: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn links_of(tx: &rusqlite::Connection, id: KbId) -> Result<Vec<KbLink>> {
    raw_links(tx, id)?
        .into_iter()
        .map(|l| {
            let target = match (&l.target_uuid, table_of(l.kind)) {
                (Some(uuid), Some(table)) => tx
                    .query_row(
                        &format!("SELECT id FROM {table} WHERE uuid = ?1"),
                        [uuid],
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?
                    .and_then(|rid| to_record(l.kind, rid))
                    .map_or(KbLinkTarget::Missing, KbLinkTarget::Record),
                _ => l
                    .target_text
                    .clone()
                    .map_or(KbLinkTarget::Missing, KbLinkTarget::Text),
            };
            Ok(KbLink {
                id: l.id,
                kind: l.kind,
                target,
                folder: l.folder.map(FolderId),
                blob: l.blob,
                created_at: l.created_at,
                created_by: l.created_by,
            })
        })
        .collect()
}

/// Copies one source to another entry unless it names it already.
fn copy_link(
    tx: &rusqlite::Connection,
    link: KbLink,
    to: KbId,
    author: Author,
    now: i64,
) -> Result<Option<i64>> {
    let raw = raw_links(tx, to)?;
    let source: Option<(Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT target_uuid, target_text FROM kb_links WHERE id = ?1",
            [link.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((uuid, text)) = source else {
        return Ok(None);
    };
    let folder = link.folder.map(|f| f.0);
    if raw.iter().any(|r| {
        r.kind == link.kind && r.target_uuid == uuid && r.target_text == text && r.folder == folder
    }) {
        return Ok(None);
    }
    tx.execute(
        "INSERT INTO kb_links (uuid, entry_id, kind, target_uuid, target_text, folder_id, blob, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![new_uuid(), to.0, link.kind, uuid, text, folder, link.blob, now, author],
    )?;
    Ok(Some(tx.last_insert_rowid()))
}
