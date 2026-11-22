//! The schema as a ladder of migrations on `PRAGMA user_version` (ADR 0003).
//!
//! Additive only between releases: new tables, new nullable or defaulted
//! columns, new indexes. No rename, no drop, no change of meaning. Code never
//! uses `SELECT *` or positional inserts, so a newer schema stays readable.
//!
//! Every table: `id INTEGER PRIMARY KEY` (local; feeds short ids and FTS
//! rowids), `uuid` (UUIDv7, the identity across export and import),
//! `created_at`, `updated_at`, `updated_by`, `deleted_at`, `deleted_by`,
//! `deleted_batch` (the delete that trashed it; local, not exported)
//! (integer Unix ms; null `deleted_at` means live). Knowledge entries and
//! folders belong to exactly one owner: a project, or an epic, or a topic;
//! `project_id` is always set so roll-up queries need no join.

/// Step `i` takes `user_version` from `i` to `i + 1`.
pub const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7];

/// The version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// The first version with attachments: from it on the database runs with
/// `auto_vacuum = INCREMENTAL`, set by one `VACUUM` after the step (it
/// cannot run inside the step's transaction; see `db::migrate`).
pub const INCREMENTAL_VACUUM_FROM: i64 = 6;

const V1: &str = r#"
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT
) STRICT;

CREATE TABLE epics (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL,
    include_inherited_folders INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT
) STRICT;
CREATE INDEX epics_project ON epics(project_id, position);

CREATE TABLE topics (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    epic_id INTEGER REFERENCES epics(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT '',
    state_updated_at INTEGER,
    state_updated_by TEXT,
    position INTEGER NOT NULL,
    include_inherited_folders INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT
) STRICT;
CREATE INDEX topics_project ON topics(project_id, epic_id, position);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    seq INTEGER,
    kind TEXT NOT NULL,
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    event TEXT,
    old_value TEXT,
    new_value TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT
) STRICT;
CREATE INDEX entries_topic ON entries(topic_id, created_at);
-- One number per entry in a topic, even after a merge import.
CREATE UNIQUE INDEX entries_topic_seq ON entries(topic_id, seq) WHERE seq IS NOT NULL;

CREATE TABLE notes (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    text TEXT NOT NULL,
    author TEXT NOT NULL,
    filed_topic_id INTEGER REFERENCES topics(id),
    filed_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT
) STRICT;
CREATE INDEX notes_project ON notes(project_id, created_at);

CREATE TABLE kb_entries (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    epic_id INTEGER REFERENCES epics(id),
    topic_id INTEGER REFERENCES topics(id),
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    body TEXT NOT NULL,
    rev INTEGER NOT NULL,
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT,
    CHECK (epic_id IS NULL OR topic_id IS NULL)
) STRICT;
CREATE INDEX kb_project ON kb_entries(project_id);
CREATE INDEX kb_epic ON kb_entries(epic_id);
CREATE INDEX kb_topic ON kb_entries(topic_id);

CREATE TABLE kb_revisions (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    entry_id INTEGER NOT NULL REFERENCES kb_entries(id),
    rev INTEGER NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    body TEXT NOT NULL,
    author TEXT NOT NULL,
    change TEXT NOT NULL,
    section TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE (entry_id, rev)
) STRICT;

CREATE TABLE folders (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    epic_id INTEGER REFERENCES epics(id),
    topic_id INTEGER REFERENCES topics(id),
    path TEXT NOT NULL,
    position INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT,
    CHECK (epic_id IS NULL OR topic_id IS NULL)
) STRICT;
CREATE INDEX folders_owner ON folders(project_id, epic_id, topic_id, position);

-- Full-text search: external-content FTS5 tables kept in sync by triggers.
-- Soft-deleted rows stay indexed; queries join the base table on deleted_at.
CREATE VIRTUAL TABLE topics_fts USING fts5(
    title, description, state,
    content='topics', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER topics_fts_ai AFTER INSERT ON topics BEGIN
    INSERT INTO topics_fts(rowid, title, description, state)
    VALUES (new.id, new.title, new.description, new.state);
END;
CREATE TRIGGER topics_fts_ad AFTER DELETE ON topics BEGIN
    INSERT INTO topics_fts(topics_fts, rowid, title, description, state)
    VALUES ('delete', old.id, old.title, old.description, old.state);
END;
CREATE TRIGGER topics_fts_au AFTER UPDATE OF title, description, state ON topics BEGIN
    INSERT INTO topics_fts(topics_fts, rowid, title, description, state)
    VALUES ('delete', old.id, old.title, old.description, old.state);
    INSERT INTO topics_fts(rowid, title, description, state)
    VALUES (new.id, new.title, new.description, new.state);
END;

CREATE VIRTUAL TABLE entries_fts USING fts5(
    text, content='entries', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER entries_fts_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER entries_fts_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER entries_fts_au AFTER UPDATE OF text ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO entries_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE VIRTUAL TABLE notes_fts USING fts5(
    text, content='notes', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER notes_fts_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER notes_fts_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER notes_fts_au AFTER UPDATE OF text ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO notes_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE VIRTUAL TABLE kb_fts USING fts5(
    title, summary, body,
    content='kb_entries', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER kb_fts_ai AFTER INSERT ON kb_entries BEGIN
    INSERT INTO kb_fts(rowid, title, summary, body)
    VALUES (new.id, new.title, new.summary, new.body);
END;
CREATE TRIGGER kb_fts_ad AFTER DELETE ON kb_entries BEGIN
    INSERT INTO kb_fts(kb_fts, rowid, title, summary, body)
    VALUES ('delete', old.id, old.title, old.summary, old.body);
END;
CREATE TRIGGER kb_fts_au AFTER UPDATE OF title, summary, body ON kb_entries BEGIN
    INSERT INTO kb_fts(kb_fts, rowid, title, summary, body)
    VALUES ('delete', old.id, old.title, old.summary, old.body);
    INSERT INTO kb_fts(rowid, title, summary, body)
    VALUES (new.id, new.title, new.summary, new.body);
END;
"#;

/// M6: a note can become an epic, and the inbox shows who filed each note.
/// A note with `filed_at` set and no target was dismissed.
const V2: &str = r#"
ALTER TABLE notes ADD COLUMN filed_epic_id INTEGER REFERENCES epics(id);
ALTER TABLE notes ADD COLUMN filed_by TEXT;
CREATE INDEX notes_filed_topic ON notes(filed_topic_id) WHERE filed_topic_id IS NOT NULL;
"#;

/// M7: read markers (docs/design/data.md, request 1). When this
/// installation last showed a topic (`kind = 'topic'`) or a knowledge entry
/// (`kind = 'kb'`). Local: never exported or imported, never part of a
/// record's stamp. Records that exist when the step runs count as seen, so
/// an update does not light every row.
const V3: &str = r#"
CREATE TABLE read_markers (
    kind TEXT NOT NULL CHECK (kind IN ('topic', 'kb')),
    record_id INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    PRIMARY KEY (kind, record_id)
) WITHOUT ROWID;
INSERT INTO read_markers (kind, record_id, last_seen_at)
    SELECT 'topic', id, updated_at FROM topics;
INSERT INTO read_markers (kind, record_id, last_seen_at)
    SELECT 'kb', id, updated_at FROM kb_entries;
"#;

/// Folder sets and commit modes (ADR 0011), due dates (data.md request 16)
/// and the Overview's index (request 17), in one additive step.
///
/// - Folders get a name (`''` = the last segment of the path), an icon
///   (`''` = automatic), a commit mode and its two options
///   (`commit_as_user` NULL = the Settings default).
/// - Projects, epics and topics get an optional pointer to their active
///   folder. No pointer is set: the default rule (the first own folder, else
///   the parent's active folder) gives every page its old primary folder.
///   `include_inherited_folders` stays, no longer read or written.
/// - Projects, epics and topics get an optional due day, `YYYY-MM-DD` in
///   2000 to 2099. No backfill.
/// - `entries_recent` holds only the Overview's "last 7 days" rows;
///   `entries_live` gives a topic's last activity from the index alone.
const V4: &str = r#"
ALTER TABLE folders ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE folders ADD COLUMN icon TEXT NOT NULL DEFAULT '';
ALTER TABLE folders ADD COLUMN commit_mode TEXT NOT NULL DEFAULT 'manual'
    CHECK (commit_mode IN ('manual', 'commit', 'push'));
ALTER TABLE folders ADD COLUMN commit_as_user INTEGER;
ALTER TABLE folders ADD COLUMN commit_on_default INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);
ALTER TABLE epics ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);
ALTER TABLE topics ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);

ALTER TABLE projects ADD COLUMN due_on TEXT
    CHECK (due_on IS NULL OR due_on GLOB '20[0-9][0-9]-[01][0-9]-[0-3][0-9]');
ALTER TABLE epics ADD COLUMN due_on TEXT
    CHECK (due_on IS NULL OR due_on GLOB '20[0-9][0-9]-[01][0-9]-[0-3][0-9]');
ALTER TABLE topics ADD COLUMN due_on TEXT
    CHECK (due_on IS NULL OR due_on GLOB '20[0-9][0-9]-[01][0-9]-[0-3][0-9]');
CREATE INDEX topics_due ON topics(project_id, due_on) WHERE due_on IS NOT NULL;
CREATE INDEX epics_due ON epics(project_id, due_on) WHERE due_on IS NOT NULL;

CREATE INDEX entries_recent ON entries(created_at)
    WHERE deleted_at IS NULL AND (kind = 'decision' OR (kind = 'system' AND event = 'status'));
-- The newest live entry of a topic from the index alone (the Overview's
-- "no activity" line): entries_topic needs a row read per entry for
-- deleted_at, which missed the 5 ms budget on 10,000 topics.
CREATE INDEX entries_live ON entries(topic_id, created_at) WHERE deleted_at IS NULL;
"#;

/// Questions (data.md request 18), contacts with Report to (requests 19
/// and 22) and the profile (request 23), in one additive step.
///
/// - `contacts`: a person work is asked of or reported to, at one level:
///   all projects (`project_id` and `epic_id` null), a project, or an epic
///   of that project. `name_key` is `person::person_key(name)`, written by
///   the store, never read from a file. `merged_into` names the contact a
///   merged one went into (the trash row).
/// - `contact_distinct`: pairs the human kept apart ("Keep both").
/// - `profile`: the human's own name and job title; one row.
/// - `questions`: exactly one owner, as knowledge entries and folders
///   have it (`project_id` always set; `epic_id` or `topic_id` for an
///   epic or a topic), the text, the contact asked, the day the answer is
///   expected, the state, the answer, who resolved it and when, the
///   decision entry logged with the answer, the stored order.
/// - Notes, topics and epics name the contact to report to; topics and
///   epics also when it was reported and by whom. Projects have none.
/// - `record_markers`: when this installation last showed a question or
///   a contact (request 1); local, never exported. Neither exists before
///   this step, so nothing is seeded.
const V5: &str = r#"
CREATE TABLE contacts (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER REFERENCES projects(id),
    epic_id INTEGER REFERENCES epics(id),
    name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    job_title TEXT,
    company TEXT,
    address_note TEXT,
    merged_into INTEGER REFERENCES contacts(id),
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT,
    CHECK (epic_id IS NULL OR project_id IS NOT NULL)
) STRICT;
CREATE INDEX contacts_key ON contacts(name_key) WHERE deleted_at IS NULL;
CREATE INDEX contacts_level ON contacts(project_id, epic_id) WHERE deleted_at IS NULL;

CREATE TABLE contact_distinct (
    a INTEGER NOT NULL REFERENCES contacts(id),
    b INTEGER NOT NULL REFERENCES contacts(id),
    PRIMARY KEY (a, b),
    CHECK (a < b)
) WITHOUT ROWID;

CREATE TABLE profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    name TEXT,
    job_title TEXT,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE questions (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    epic_id INTEGER REFERENCES epics(id),
    topic_id INTEGER REFERENCES topics(id),
    text TEXT NOT NULL,
    asked_contact_id INTEGER REFERENCES contacts(id),
    expected_on TEXT
        CHECK (expected_on IS NULL OR expected_on GLOB '20[0-9][0-9]-[01][0-9]-[0-3][0-9]'),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'answered', 'dropped')),
    answer TEXT,
    resolved_at INTEGER,
    resolved_by TEXT,
    decision_entry_id INTEGER REFERENCES entries(id) ON DELETE SET NULL,
    position INTEGER NOT NULL,
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT,
    CHECK (epic_id IS NULL OR topic_id IS NULL)
) STRICT;
CREATE INDEX questions_owner ON questions(project_id, epic_id, topic_id, status, position);
CREATE INDEX questions_due ON questions(project_id, status, expected_on);
CREATE INDEX questions_contact ON questions(project_id, asked_contact_id)
    WHERE asked_contact_id IS NOT NULL;

CREATE TABLE record_markers (
    kind TEXT NOT NULL CHECK (kind IN ('question', 'contact')),
    record_id INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    PRIMARY KEY (kind, record_id)
) WITHOUT ROWID;

ALTER TABLE notes ADD COLUMN report_contact_id INTEGER REFERENCES contacts(id);
ALTER TABLE topics ADD COLUMN report_contact_id INTEGER REFERENCES contacts(id);
ALTER TABLE topics ADD COLUMN reported_at INTEGER;
ALTER TABLE topics ADD COLUMN reported_by TEXT;
ALTER TABLE epics ADD COLUMN report_contact_id INTEGER REFERENCES contacts(id);
ALTER TABLE epics ADD COLUMN reported_at INTEGER;
ALTER TABLE epics ADD COLUMN reported_by TEXT;
CREATE INDEX notes_report ON notes(project_id, report_contact_id) WHERE report_contact_id IS NOT NULL;
CREATE INDEX topics_report ON topics(project_id, report_contact_id) WHERE report_contact_id IS NOT NULL;
CREATE INDEX epics_report ON epics(project_id, report_contact_id) WHERE report_contact_id IS NOT NULL;
"#;

/// Attachments (ADR 0012, data.md request 20).
///
/// - `attachment_blobs`: one row per distinct content, keyed by the
///   SHA-256 of the plaintext (hex; only ever inside the encrypted
///   database). A blob being written carries `pending:<uuid>` and
///   `complete = 0` until its last chunk is in; a crash leaves such a row,
///   which the purge removes. Images keep their pixel size and a 256 px
///   PNG thumbnail.
/// - `attachment_chunks`: the content, 1 MiB per row, so writes, reads and
///   exports stream with bounded memory and no transaction spans a file.
/// - `attachments`: one owner (a note, an entry, or a topic's or an
///   epic's description), the blob, the name as given, the order, the note
///   a filing moved it from, and the usual stamp and trash columns.
/// - `notes.filed_entry_id`: the entry a note was filed as, so its files
///   follow it there again when an undone filing is redone.
const V6: &str = r#"
CREATE TABLE attachment_blobs (
    id INTEGER PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    thumbnail BLOB,
    complete INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE attachment_chunks (
    id INTEGER PRIMARY KEY,
    blob_id INTEGER NOT NULL REFERENCES attachment_blobs(id),
    seq INTEGER NOT NULL,
    data BLOB NOT NULL,
    UNIQUE (blob_id, seq)
) STRICT;

CREATE TABLE attachments (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    note_id INTEGER REFERENCES notes(id),
    entry_id INTEGER REFERENCES entries(id),
    topic_id INTEGER REFERENCES topics(id),
    epic_id INTEGER REFERENCES epics(id),
    blob_id INTEGER NOT NULL REFERENCES attachment_blobs(id),
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    moved_from_note_id INTEGER REFERENCES notes(id),
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    updated_by TEXT NOT NULL,
    deleted_at INTEGER,
    deleted_by TEXT,
    deleted_batch TEXT,
    CHECK ((note_id IS NOT NULL) + (entry_id IS NOT NULL) + (topic_id IS NOT NULL)
           + (epic_id IS NOT NULL) = 1)
) STRICT;
CREATE INDEX attachments_note ON attachments(note_id, position) WHERE note_id IS NOT NULL;
CREATE INDEX attachments_entry ON attachments(entry_id, position) WHERE entry_id IS NOT NULL;
CREATE INDEX attachments_topic ON attachments(topic_id, position) WHERE topic_id IS NOT NULL;
CREATE INDEX attachments_epic ON attachments(epic_id, position) WHERE epic_id IS NOT NULL;
CREATE INDEX attachments_project ON attachments(project_id);
CREATE INDEX attachments_blob ON attachments(blob_id);
CREATE INDEX attachments_moved ON attachments(moved_from_note_id)
    WHERE moved_from_note_id IS NOT NULL;

ALTER TABLE notes ADD COLUMN filed_entry_id INTEGER REFERENCES entries(id);
"#;

/// Memory quality (ADR 0015, 9; data.md request 24).
///
/// - `kb_entries`: the verified date (backfilled from the last update, the
///   best value there is), the state (`live`, `superseded`, `retired`) and
///   the successor, the context narrowing (a branch of a folder) and the
///   provenance of an agent write (the Claude Code session id, local and
///   never exported, and the focus topic). `kb_verified` serves the
///   Overview's count of memories to check.
/// - `kb_revisions`: the session and branch of each write.
/// - `kb_links`: the sources an entry names: a record (`target_uuid`), a
///   commit or a file (`target_text`, with the folder and the git blob id
///   at the last verification).
/// - `kb_pairs`: duplicate and conflict pairs found by the checker, open,
///   kept apart (`distinct`) or settled (`resolved`); `a < b`.
/// - `kb_quality_log`: warnings and their outcomes for `acta kb quality`.
///   Local: never exported, pruned after 180 days.
const V7: &str = r#"
ALTER TABLE kb_entries ADD COLUMN verified_at INTEGER;
ALTER TABLE kb_entries ADD COLUMN verified_by TEXT;
ALTER TABLE kb_entries ADD COLUMN state TEXT NOT NULL DEFAULT 'live'
    CHECK (state IN ('live', 'superseded', 'retired'));
ALTER TABLE kb_entries ADD COLUMN superseded_by INTEGER REFERENCES kb_entries(id);
ALTER TABLE kb_entries ADD COLUMN context_branch TEXT;
ALTER TABLE kb_entries ADD COLUMN context_folder_id INTEGER REFERENCES folders(id);
ALTER TABLE kb_entries ADD COLUMN created_session TEXT;
ALTER TABLE kb_entries ADD COLUMN created_topic_id INTEGER REFERENCES topics(id);
UPDATE kb_entries SET verified_at = updated_at, verified_by = updated_by;
CREATE INDEX kb_verified ON kb_entries(project_id, kind, verified_at)
    WHERE deleted_at IS NULL AND state = 'live';
CREATE INDEX kb_superseded ON kb_entries(superseded_by) WHERE superseded_by IS NOT NULL;

ALTER TABLE kb_revisions ADD COLUMN session TEXT;
ALTER TABLE kb_revisions ADD COLUMN branch TEXT;

CREATE TABLE kb_links (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    entry_id INTEGER NOT NULL REFERENCES kb_entries(id),
    kind TEXT NOT NULL
        CHECK (kind IN ('topic', 'entry', 'question', 'note', 'kb', 'commit', 'file')),
    target_uuid TEXT,
    target_text TEXT,
    folder_id INTEGER REFERENCES folders(id),
    blob TEXT,
    created_at INTEGER NOT NULL,
    created_by TEXT NOT NULL,
    CHECK ((target_uuid IS NULL) <> (target_text IS NULL))
) STRICT;
CREATE INDEX kb_links_entry ON kb_links(entry_id);

CREATE TABLE kb_pairs (
    a INTEGER NOT NULL REFERENCES kb_entries(id),
    b INTEGER NOT NULL REFERENCES kb_entries(id),
    kind TEXT NOT NULL CHECK (kind IN ('duplicate', 'conflict')),
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'distinct', 'resolved')),
    score REAL NOT NULL,
    created_at INTEGER NOT NULL,
    created_by TEXT NOT NULL,
    resolved_at INTEGER,
    resolved_by TEXT,
    PRIMARY KEY (a, b, kind),
    CHECK (a < b)
) STRICT, WITHOUT ROWID;
CREATE INDEX kb_pairs_b ON kb_pairs(b);

CREATE TABLE kb_quality_log (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    event TEXT NOT NULL,
    entry_id INTEGER,
    other_id INTEGER,
    session TEXT,
    author TEXT NOT NULL
) STRICT;
CREATE INDEX kb_quality_log_at ON kb_quality_log(at);
"#;
