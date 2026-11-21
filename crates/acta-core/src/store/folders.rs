//! Working folders (ADR 0008, as amended by ADR 0011): ordered lists on
//! projects, epics and topics, always inherited, one active folder per page,
//! and the reverse lookup from a directory to the deepest attached folder.

use super::{STAMP_COLS, Store, new_uuid, owner_at, owner_condition, owner_ids, stamp};
use crate::error::{Error, Result};
use crate::folder_identity::{self, Followed, Known, Learned};
use crate::folder_path;
use crate::model::{
    Author, CommitMode, EpicId, FOLDER_ICONS, Folder, FolderId, Owner, ProjectId, Record, TopicId,
};
use crate::paths::Os;
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};
use std::path::Path;

/// One folder a page sees (ADR 0011, 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleFolder {
    pub folder: Folder,
    /// The level the folder belongs to: the page itself, its epic or its
    /// project. Inherited folders are edited there.
    pub origin: Owner,
    /// Claude Code starts here; every other folder is additional.
    pub active: bool,
}

/// The folders a page sees: its own, then its epic's own, then its
/// project's own, each group in its order; a path present at two levels is
/// listed once, at the nearest. At most one is active.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisibleFolders {
    pub folders: Vec<VisibleFolder>,
}

impl VisibleFolders {
    /// The active folder: the page's pointer when it points at a folder the
    /// page sees, else the default rule (the page's first own folder, else
    /// its parent's active folder). `None` only when the page sees none.
    pub fn active(&self) -> Option<&VisibleFolder> {
        self.folders.iter().find(|f| f.active)
    }

    /// Every folder but the active one, in list order (the `--add-dir`
    /// candidates, before the same-repository rule of ADR 0011).
    pub fn additional(&self) -> impl Iterator<Item = &VisibleFolder> {
        self.folders.iter().filter(|f| !f.active)
    }

    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }
}

/// What a directory resolves to, for sessions started outside Acta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderResolution {
    /// The deepest attached folder containing the directory.
    pub folder: String,
    pub project: ProjectId,
    pub epic: Option<EpicId>,
    pub topic: Option<TopicId>,
    /// When the folder is attached to several topics, the focus falls back
    /// to their common epic or the project, and these are the candidates.
    pub candidates: Vec<TopicId>,
}

/// Local-value prefix of the folders where Acta turned Claude Code's auto
/// memory off (`acta link`, ADR 0005), followed by the folder's stored path.
pub const AUTO_MEMORY_PREFIX: &str = "automemory.";

/// Local-value prefix of a folder's identity (ADR 0008, "Live files"),
/// followed by its local id: never exported, like the path's meaning.
const IDENTITY_PREFIX: &str = "folderid.";

/// Local values kept per folder path (ADR 0008 "Live files"): the
/// auto-memory marker of earlier versions and Claude Code's settings ledger
/// (ADR 0011). They follow the path whenever a folder's path changes.
const PATH_KEYED: [&str; 2] = [AUTO_MEMORY_PREFIX, crate::claude_settings::LEDGER_PREFIX];

/// Carries the path-keyed values from `old` to `new` after folder `id`
/// moved there: moved when no other live folder still has `old` (one path
/// may be attached at several levels), copied otherwise. A value already
/// under `new` is kept.
fn carry_path_keys(
    conn: &rusqlite::Connection,
    id: FolderId,
    old: &str,
    new: &str,
) -> rusqlite::Result<()> {
    if old == new {
        return Ok(());
    }
    let others: i64 = conn.query_row(
        "SELECT COUNT(*) FROM folders WHERE path = ?1 AND id != ?2 AND deleted_at IS NULL",
        params![old, id.0],
        |r| r.get(0),
    )?;
    for prefix in PATH_KEYED {
        let (from, to) = (
            format!("local.{prefix}{old}"),
            format!("local.{prefix}{new}"),
        );
        if others > 0 {
            conn.execute(
                "INSERT OR IGNORE INTO meta (key, value) SELECT ?2, value FROM meta WHERE key = ?1",
                params![from, to],
            )?;
        } else {
            conn.execute(
                "UPDATE OR IGNORE meta SET key = ?2 WHERE key = ?1",
                params![from, to],
            )?;
        }
    }
    Ok(())
}

fn identity_key(id: FolderId) -> String {
    format!("local.{IDENTITY_PREFIX}{}", id.0)
}

/// Forgets the identity kept under a folder id: a row inserted by an import
/// must not inherit one left by an earlier row with the same id.
pub(crate) fn forget_identity(
    conn: &rusqlite::Connection,
    id: FolderId,
) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM meta WHERE key = ?1", [identity_key(id)])
}

fn upsert_meta(conn: &rusqlite::Connection, key: &str, value: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
}

const FOLDER_COLS: &str = "id, project_id, epic_id, topic_id, path, position, \
     name, icon, commit_mode, commit_as_user, commit_on_default";

/// Reads [`FOLDER_COLS`] then [`STAMP_COLS`].
fn folder_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: FolderId(r.get(0)?),
        project: ProjectId(r.get(1)?),
        owner: owner_at(r, 1)?,
        path: r.get(4)?,
        position: r.get(5)?,
        name: r.get(6)?,
        icon: r.get(7)?,
        commit_mode: r.get(8)?,
        commit_as_user: r.get(9)?,
        commit_on_default: r.get(10)?,
        stamp: stamp(r, 11)?,
    })
}

/// The page and the levels above it, nearest first, each with its pointer.
fn chain(conn: &rusqlite::Connection, owner: Owner) -> Result<Vec<(Owner, Option<FolderId>)>> {
    use super::records::{read_epic, read_project, read_topic};
    let project = |id: ProjectId| -> Result<(Owner, Option<FolderId>)> {
        Ok((Owner::Project(id), read_project(conn, id)?.active_folder))
    };
    let epic = |id: EpicId| -> Result<Vec<(Owner, Option<FolderId>)>> {
        let e = read_epic(conn, id)?;
        Ok(vec![
            (Owner::Epic(id), e.active_folder),
            project(e.project)?,
        ])
    };
    Ok(match owner {
        Owner::Project(id) => vec![project(id)?],
        Owner::Epic(id) => epic(id)?,
        Owner::Topic(id) => {
            let t = read_topic(conn, id)?;
            let mut levels = vec![(owner, t.active_folder)];
            match t.epic {
                Some(e) => levels.extend(epic(e)?),
                None => levels.push(project(t.project)?),
            }
            levels
        }
    })
}

/// The folders `owner` sees and its active one (ADR 0011, 1).
fn visible(conn: &rusqlite::Connection, owner: Owner) -> Result<VisibleFolders> {
    let os = Os::CURRENT;
    let levels = chain(conn, owner)?;
    let own: Vec<Vec<Folder>> = levels
        .iter()
        .map(|(level, _)| folders_of(conn, *level))
        .collect::<Result<_>>()?;
    // The listed folders and, for a folder hidden by a nearer one with the
    // same path, the one listed in its place.
    let mut listed: Vec<VisibleFolder> = Vec::new();
    let mut alias: Vec<(FolderId, FolderId)> = Vec::new();
    for ((level, _), folders) in levels.iter().zip(&own) {
        for f in folders {
            match listed
                .iter()
                .find(|l| folder_path::same(os, &l.folder.path, &f.path))
            {
                Some(kept) => alias.push((f.id, kept.folder.id)),
                None => listed.push(VisibleFolder {
                    folder: f.clone(),
                    origin: *level,
                    active: false,
                }),
            }
        }
    }
    // From the project down: each level's choice when it sees the folder,
    // else its first own folder, else what the level above resolved.
    let mut active: Option<FolderId> = None;
    for (i, (_, pointer)) in levels.iter().enumerate().rev() {
        let sees = |id: FolderId| own[i..].iter().flatten().any(|f| f.id == id);
        active = match pointer {
            Some(p) if sees(*p) => Some(*p),
            _ => own[i].first().map(|f| f.id).or(active),
        };
    }
    if let Some(id) = active {
        let id = alias
            .iter()
            .find(|(hidden, _)| *hidden == id)
            .map_or(id, |(_, kept)| *kept);
        if let Some(f) = listed.iter_mut().find(|f| f.folder.id == id) {
            f.active = true;
        }
    }
    Ok(VisibleFolders { folders: listed })
}

impl Store {
    /// Attaches an existing folder, stored in canonical form, at the end of
    /// the owner's list. The same folder twice on one owner is refused.
    pub fn add_folder(&mut self, owner: Owner, path: &Path, author: Author) -> Result<FolderId> {
        let stored = folder_path::canonical(path)
            .map_err(|e| Error::Invalid(format!("cannot attach this folder: {e}")))?;
        let id = self.add_folder_stored(owner, &stored, author)?;
        self.remember_identity(id, &stored);
        Ok(id)
    }

    /// Reads the identity of the folder now at `stored` and keeps it, so a
    /// later rename or move can be followed. Best effort: without one, the
    /// monitor learns it on its next pass.
    fn remember_identity(&mut self, id: FolderId, stored: &str) {
        let Some(identity) = folder_identity::identity(Path::new(stored)) else {
            return;
        };
        let _ = self.write().and_then(|tx| {
            upsert_meta(&tx, &identity_key(id), &identity.encode())?;
            tx.commit()?;
            Ok(())
        });
    }

    pub(crate) fn add_folder_stored(
        &mut self,
        owner: Owner,
        stored: &str,
        author: Author,
    ) -> Result<FolderId> {
        let cond = owner_condition(owner);
        let tx = self.write()?;
        let (project, epic, topic) = owner_ids(&tx, owner)?;
        if folders_of(&tx, owner)?
            .iter()
            .any(|f| folder_path::same(Os::CURRENT, &f.path, stored))
        {
            return Err(Error::Invalid(format!(
                "this folder is already attached to {owner}"
            )));
        }
        let now = now_ms();
        let position: i64 = tx.query_row(
            &format!("SELECT coalesce(max(position) + 1, 0) FROM folders WHERE {cond} AND deleted_at IS NULL"),
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO folders (uuid, project_id, epic_id, topic_id, path, position, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)",
            params![new_uuid(), project, epic, topic, stored, position, now, author],
        )?;
        let id = FolderId(tx.last_insert_rowid());
        tx.commit()?;
        let _ = self.refresh_link_index();
        Ok(id)
    }

    pub fn folder(&self, id: FolderId) -> Result<Folder> {
        read_folder(&self.conn, id)
    }

    /// The owner's own folders, in order.
    pub fn folders(&self, owner: Owner) -> Result<Vec<Folder>> {
        folders_of(&self.conn, owner)
    }

    /// Every live folder of every live owner, for the "folders not found"
    /// list and bulk repair (request 6). Checking that each exists is the
    /// caller's job, off the UI thread.
    pub fn all_folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {FOLDER_COLS}, {STAMP_COLS} FROM folders
             WHERE deleted_at IS NULL ORDER BY project_id, id"
        ))?;
        let rows = stmt.query_map([], folder_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Points a folder at another existing folder (repair after an import).
    pub fn set_folder_path(&mut self, id: FolderId, path: &Path, author: Author) -> Result<()> {
        let old = self.folder(id)?.path;
        let stored = folder_path::canonical(path)
            .map_err(|e| Error::Invalid(format!("cannot use this folder: {e}")))?;
        let tx = self.write()?;
        tx.execute(
            "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, stored, now_ms(), author],
        )?;
        carry_path_keys(&tx, id, &old, &stored)?;
        tx.commit()?;
        let _ = self.refresh_link_index();
        // Found again: its Claude Code settings are written now.
        let _ = crate::claude_settings::sync_path(self, Path::new(&stored));
        self.remember_identity(id, &stored);
        Ok(())
    }

    /// Sets several folders to paths already in stored form, in one
    /// transaction (bulk repair: the caller resolved the paths off the
    /// store, so nothing here touches the disk). Each path must already be
    /// in the normal form of ADR 0008 (what `folder_path::canonical`
    /// returns) and must not be another folder of the same owner, after the
    /// change; otherwise it is refused and nothing is written.
    pub fn set_folder_paths(
        &mut self,
        changes: &[(FolderId, String)],
        author: Author,
    ) -> Result<()> {
        let os = Os::CURRENT;
        let tx = self.write()?;
        let now = now_ms();
        for (id, path) in changes {
            if folder_path::normalize(os, path) != *path {
                return Err(Error::Invalid(format!("{id}: path is not in stored form")));
            }
            let folder = read_folder(&tx, *id)?;
            let siblings: Vec<(i64, String)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT id, path FROM folders WHERE {} AND deleted_at IS NULL",
                    super::owner_condition(folder.owner)
                ))?;
                stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?
            };
            let clash = siblings.iter().any(|(other, other_path)| {
                let after = changes
                    .iter()
                    .find(|(c, _)| c.0 == *other)
                    .map_or(other_path.as_str(), |(_, p)| p.as_str());
                *other != id.0 && folder_path::same(os, after, path)
            });
            if clash {
                return Err(Error::Invalid(format!(
                    "{id}: the same folder is already attached here"
                )));
            }
            tx.execute(
                "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
                params![id.0, path, now, author],
            )?;
            carry_path_keys(&tx, *id, &folder.path, path)?;
            // Another folder now: its identity is learned on the next pass.
            tx.execute("DELETE FROM meta WHERE key = ?1", [identity_key(*id)])?;
        }
        tx.commit()?;
        let _ = self.refresh_link_index();
        for (_, path) in changes {
            let _ = crate::claude_settings::sync_path(self, Path::new(path));
        }
        Ok(())
    }

    /// Puts back a path exactly as it was stored, without checking that it
    /// exists: the undo of a Locate, whose old path is by definition gone.
    pub fn restore_folder_path(
        &mut self,
        id: FolderId,
        stored: &str,
        author: Author,
    ) -> Result<()> {
        let old = self.folder(id)?.path;
        let tx = self.write()?;
        tx.execute(
            "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, stored, now_ms(), author],
        )?;
        carry_path_keys(&tx, id, &old, stored)?;
        tx.execute("DELETE FROM meta WHERE key = ?1", [identity_key(id)])?;
        tx.commit()?;
        let _ = self.refresh_link_index();
        Ok(())
    }

    /// Every live folder with its identity, for the folder monitor
    /// ([`folder_identity::Monitor`]).
    pub fn monitored_folders(&self) -> Result<Vec<Known>> {
        let identities: std::collections::HashMap<String, String> = self
            .local_values(IDENTITY_PREFIX)?
            .into_iter()
            .map(|(k, v)| (k[IDENTITY_PREFIX.len()..].to_owned(), v))
            .collect();
        Ok(self
            .all_folders()?
            .into_iter()
            .map(|f| Known {
                identity: identities.get(&f.id.0.to_string()).cloned(),
                id: f.id,
                path: f.path,
            })
            .collect())
    }

    /// Writes what a monitor pass found, in one transaction: identities
    /// learned (only for a folder still at the path it was read at), and
    /// folders followed to where they were moved. Identities kept for ids
    /// that no longer name a folder row are dropped. A followed
    /// folder keeps everything but its path, and what depends on the path
    /// follows it: the auto-memory marker of `acta link` and the hooks'
    /// pre-check file. A folder whose path changed since the pass, or whose
    /// new path is already attached to the same owner, is left as it is.
    /// Returns the folders followed. Touches no disk but the pre-check file.
    pub fn apply_folder_pass(
        &mut self,
        learned: &[Learned],
        followed: &[Followed],
    ) -> Result<Vec<FolderId>> {
        if learned.is_empty() && followed.is_empty() {
            return Ok(Vec::new());
        }
        let os = Os::CURRENT;
        let tx = self.write()?;
        for l in learned {
            if read_folder(&tx, l.id).is_ok_and(|f| f.path == l.path) {
                upsert_meta(&tx, &identity_key(l.id), &l.identity)?;
            }
        }
        tx.execute(
            &format!(
                "DELETE FROM meta WHERE substr(key, 1, {n}) = 'local.{IDENTITY_PREFIX}'
                 AND CAST(substr(key, {n} + 1) AS INTEGER) NOT IN (SELECT id FROM folders)",
                n = "local.".len() + IDENTITY_PREFIX.len()
            ),
            [],
        )?;
        let mut moved = Vec::new();
        let now = now_ms();
        for f in followed {
            let Ok(folder) = read_folder(&tx, f.id) else {
                continue;
            };
            if folder.path != f.old || folder_path::normalize(os, &f.new) != f.new {
                continue;
            }
            let clash = folders_of(&tx, folder.owner)?
                .iter()
                .any(|o| o.id != f.id && folder_path::same(os, &o.path, &f.new));
            if clash {
                continue;
            }
            tx.execute(
                "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
                params![f.id.0, f.new, now, Author::Human],
            )?;
            upsert_meta(&tx, &identity_key(f.id), &f.identity)?;
            carry_path_keys(&tx, f.id, &f.old, &f.new)?;
            moved.push(f.id);
        }
        tx.commit()?;
        if !moved.is_empty() {
            let _ = self.refresh_link_index();
        }
        Ok(moved)
    }

    /// Bulk repair (ADR 0008): every folder under `old_root` whose path with
    /// `new_root` substituted exists on this machine is moved there. Returns
    /// the folders changed.
    pub fn relocate_folders(
        &mut self,
        old_root: &str,
        new_root: &Path,
        author: Author,
    ) -> Result<Vec<FolderId>> {
        let os = Os::CURRENT;
        let old_root = folder_path::normalize(os, old_root);
        let mut changes = Vec::new();
        for f in self.all_folders()? {
            if folder_path::contains(os, &old_root, &f.path).is_none() {
                continue;
            }
            let Some(rest) = f.path.get(old_root.len()..) else {
                continue;
            };
            let rest = rest.trim_start_matches(['/', '\\']);
            let candidate = if rest.is_empty() {
                new_root.to_path_buf()
            } else {
                new_root.join(rest)
            };
            if let Ok(stored) = folder_path::canonical(&candidate) {
                changes.push((f.id, f.path.clone(), stored));
            }
        }
        let tx = self.write()?;
        let now = now_ms();
        for (id, old, stored) in &changes {
            tx.execute(
                "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
                params![id.0, stored, now, author],
            )?;
            carry_path_keys(&tx, *id, old, stored)?;
            tx.execute("DELETE FROM meta WHERE key = ?1", [identity_key(*id)])?;
        }
        tx.commit()?;
        let _ = self.refresh_link_index();
        for (_, _, path) in &changes {
            let _ = crate::claude_settings::sync_path(self, Path::new(path));
        }
        Ok(changes.into_iter().map(|(id, _, _)| id).collect())
    }

    /// The folders a page sees, each with its origin, and which is active
    /// (ADR 0011, 1).
    pub fn visible_folders(&self, owner: Owner) -> Result<VisibleFolders> {
        visible(&self.conn, owner)
    }

    /// Makes a folder the page sees its active folder (`Some`), or goes back
    /// to the default rule (`None`). One write, carried by an export.
    pub fn set_active_folder(
        &mut self,
        owner: Owner,
        folder: Option<FolderId>,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let levels = chain(&tx, owner)?;
        if let Some(id) = folder {
            let mut sees = false;
            for (level, _) in &levels {
                sees |= folders_of(&tx, *level)?.iter().any(|f| f.id == id);
            }
            if !sees {
                return Err(Error::Invalid(format!("{owner} does not see {id}")));
            }
        }
        let (table, raw) = match owner {
            Owner::Project(id) => ("projects", id.0),
            Owner::Epic(id) => ("epics", id.0),
            Owner::Topic(id) => ("topics", id.0),
        };
        tx.execute(
            &format!(
                "UPDATE {table} SET active_folder_id = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1"
            ),
            params![raw, folder.map(|f| f.0), now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Names a folder; an empty name means the last segment of its path.
    pub fn set_folder_name(&mut self, id: FolderId, name: &str, author: Author) -> Result<()> {
        self.update_folder(id, "name", name.trim(), author)
    }

    /// Sets a folder's icon: one of [`FOLDER_ICONS`], or `""` for automatic.
    pub fn set_folder_icon(&mut self, id: FolderId, icon: &str, author: Author) -> Result<()> {
        if !icon.is_empty() && !FOLDER_ICONS.contains(&icon) {
            return Err(Error::Invalid(format!("unknown folder icon `{icon}`")));
        }
        self.update_folder(id, "icon", icon, author)
    }

    /// The commit mode of a folder and its two options (ADR 0011, 3):
    /// `as_user` is "Commit in my name" (`None` follows the Settings
    /// default), `on_default` is "Also on <default branch>". Commit modes
    /// are the user's: refused for Claude Code.
    pub fn set_commit_settings(
        &mut self,
        id: FolderId,
        mode: CommitMode,
        as_user: Option<bool>,
        on_default: bool,
        author: Author,
    ) -> Result<()> {
        if author == Author::Claude {
            return Err(Error::Invalid(
                "Commit modes are set by the user in Acta.".into(),
            ));
        }
        let tx = self.write()?;
        read_folder(&tx, id)?;
        tx.execute(
            "UPDATE folders SET commit_mode = ?2, commit_as_user = ?3, commit_on_default = ?4,
                    updated_at = ?5, updated_by = ?6 WHERE id = ?1",
            params![id.0, mode, as_user, on_default, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a folder up (`delta < 0`) or down (`delta > 0`) among its
    /// owner's folders, clamped at the ends (Move up / Move down).
    pub fn move_folder(&mut self, id: FolderId, delta: i64, author: Author) -> Result<()> {
        let record = Record::Folder(id);
        let at = self.order_index(record)? as i64;
        self.reorder(record, (at + delta).max(0) as usize, author)
    }

    fn update_folder(
        &mut self,
        id: FolderId,
        column: &str,
        value: &str,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        read_folder(&tx, id)?;
        tx.execute(
            &format!(
                "UPDATE folders SET {column} = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1"
            ),
            params![id.0, value, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The reverse direction (ADR 0008): the deepest attached folder that is
    /// `dir` or contains it, at any level, and the focus it gives.
    pub fn resolve_folder(&self, dir: &Path) -> Result<Option<FolderResolution>> {
        let os = Os::CURRENT;
        let stored = match folder_path::canonical(dir) {
            Ok(s) => s,
            Err(_) => folder_path::normalize(os, &dir.to_string_lossy()),
        };
        let mut best: Option<usize> = None;
        let mut hits: Vec<Folder> = Vec::new();
        for f in self.live_owner_folders()? {
            let Some(depth) = folder_path::contains(os, &f.path, &stored) else {
                continue;
            };
            match best {
                Some(b) if depth < b => {}
                Some(b) if depth == b => hits.push(f),
                _ => {
                    best = Some(depth);
                    hits = vec![f];
                }
            }
        }
        let Some(first) = hits.first() else {
            return Ok(None);
        };
        let folder = first.path.clone();
        let project = first.project;
        if hits.iter().any(|h| h.project != project) {
            // Attached in two projects: nothing can be chosen safely.
            return Ok(None);
        }
        // The most specific level wins: a topic over an epic over the
        // project. Only owners at that level can be ambiguous.
        let rank = |o: &Owner| match o {
            Owner::Topic(_) => 0,
            Owner::Epic(_) => 1,
            Owner::Project(_) => 2,
        };
        let best_rank = hits.iter().map(|h| rank(&h.owner)).min().unwrap_or(2);
        let mut owners: Vec<Owner> = Vec::new();
        for h in hits.iter().filter(|h| rank(&h.owner) == best_rank) {
            if !owners.contains(&h.owner) {
                owners.push(h.owner);
            }
        }
        if let [only] = owners[..] {
            let (epic, topic) = match only {
                Owner::Project(_) => (None, None),
                Owner::Epic(e) => (Some(e), None),
                Owner::Topic(t) => (self.topic(t)?.epic, Some(t)),
            };
            return Ok(Some(FolderResolution {
                folder,
                project,
                epic,
                topic,
                candidates: Vec::new(),
            }));
        }
        // Ambiguous: the common epic if every owner is in the same one.
        let mut epics = Vec::new();
        let mut candidates = Vec::new();
        for o in &owners {
            match *o {
                Owner::Project(_) => epics.push(None),
                Owner::Epic(e) => epics.push(Some(e)),
                Owner::Topic(t) => {
                    candidates.push(t);
                    epics.push(self.topic(t)?.epic);
                }
            }
        }
        epics.sort();
        epics.dedup();
        let epic = match epics[..] {
            [Some(e)] => Some(e),
            _ => None,
        };
        Ok(Some(FolderResolution {
            folder,
            project,
            epic,
            topic: None,
            candidates,
        }))
    }

    /// Rewrites the hooks' pre-check file (ADR 0008) from the live folders.
    /// A failure never undoes the committed write that triggered it; it is
    /// returned here and remembered for [`Store::link_index_failed`], which
    /// the CLI reports and `acta setup --check` surfaces.
    pub fn refresh_link_index(&self) -> std::io::Result<()> {
        let result = self
            .live_owner_folders()
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|folders| {
                let paths: Vec<String> = folders.into_iter().map(|f| f.path).collect();
                crate::link_index::write(&self.data_dir, Os::CURRENT, &paths)
            });
        self.link_index_failed.set(result.is_err());
        result
    }

    /// Whether the last pre-check file rewrite failed: hooks may then miss
    /// a newly attached folder until the next successful one.
    pub fn link_index_failed(&self) -> bool {
        self.link_index_failed.get()
    }

    /// Live folders whose owner chain is live.
    fn live_owner_folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.project_id, f.epic_id, f.topic_id, f.path, f.position,
                    f.name, f.icon, f.commit_mode, f.commit_as_user, f.commit_on_default,
                    f.uuid, f.created_at, f.updated_at, f.updated_by, f.deleted_at, f.deleted_by
             FROM folders f
             JOIN projects p ON p.id = f.project_id AND p.deleted_at IS NULL
             LEFT JOIN epics e ON e.id = f.epic_id
             LEFT JOIN topics t ON t.id = f.topic_id
             WHERE f.deleted_at IS NULL AND e.deleted_at IS NULL AND t.deleted_at IS NULL
             ORDER BY f.id",
        )?;
        let rows = stmt.query_map([], folder_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn folders_of(conn: &rusqlite::Connection, owner: Owner) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {FOLDER_COLS}, {STAMP_COLS} FROM folders
         WHERE {} AND deleted_at IS NULL ORDER BY position, id",
        owner_condition(owner)
    ))?;
    let rows = stmt.query_map([], folder_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn read_folder(conn: &rusqlite::Connection, id: FolderId) -> Result<Folder> {
    conn.query_row(
        &format!(
            "SELECT {FOLDER_COLS}, {STAMP_COLS} FROM folders WHERE id = ?1 AND deleted_at IS NULL"
        ),
        [id.0],
        folder_row,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}
