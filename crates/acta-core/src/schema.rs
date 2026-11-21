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
pub const MIGRATIONS: &[&str] = &[V1, V2, V3, V4];

/// The version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

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
/// - `entries_recent` holds only the Overview's "last 7 days" rows.
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
"#;
