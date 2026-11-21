//! Working folders (ADR 0008): ordered lists on projects, epics and topics,
//! nearest-wins inheritance, and the reverse lookup from a directory to the
//! deepest attached folder.

use super::{STAMP_COLS, Store, new_uuid, owner_at, owner_condition, owner_ids, stamp};
use crate::error::{Error, Result};
use crate::folder_identity::{self, Followed, Known};
use crate::folder_path;
use crate::model::{Author, EpicId, Folder, FolderId, Owner, ProjectId, TopicId};
use crate::paths::Os;
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};
use std::path::Path;

/// One effective folder and the level it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveFolder {
    pub path: String,
    pub from: Owner,
}

/// The folders a launch from `owner` uses: the first is the primary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveFolders {
    pub folders: Vec<EffectiveFolder>,
    /// Where the list comes from when the owner has none of its own
    /// ("from epic Storage"); `None` when it has its own.
    pub inherited_from: Option<Owner>,
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

fn identity_key(id: FolderId) -> String {
    format!("local.{IDENTITY_PREFIX}{}", id.0)
}

fn upsert_meta(conn: &rusqlite::Connection, key: &str, value: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
}

const FOLDER_COLS: &str = "id, project_id, epic_id, topic_id, path, position";

fn folder_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: FolderId(r.get(0)?),
        project: ProjectId(r.get(1)?),
        owner: owner_at(r, 1)?,
        path: r.get(4)?,
        position: r.get(5)?,
        stamp: stamp(r, 6)?,
    })
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
        self.folder(id)?;
        let stored = folder_path::canonical(path)
            .map_err(|e| Error::Invalid(format!("cannot use this folder: {e}")))?;
        let tx = self.write()?;
        tx.execute(
            "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, stored, now_ms(), author],
        )?;
        tx.commit()?;
        let _ = self.refresh_link_index();
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
            // Another folder now: its identity is learned on the next pass.
            tx.execute("DELETE FROM meta WHERE key = ?1", [identity_key(*id)])?;
        }
        tx.commit()?;
        let _ = self.refresh_link_index();
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
        self.folder(id)?;
        let tx = self.write()?;
        tx.execute(
            "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, stored, now_ms(), author],
        )?;
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
    /// learned, and folders followed to where they were moved. A followed
    /// folder keeps everything but its path, and what depends on the path
    /// follows it: the auto-memory marker of `acta link` and the hooks'
    /// pre-check file. A folder whose path changed since the pass, or whose
    /// new path is already attached to the same owner, is left as it is.
    /// Returns the folders followed. Touches no disk but the pre-check file.
    pub fn apply_folder_pass(
        &mut self,
        learned: &[(FolderId, String)],
        followed: &[Followed],
    ) -> Result<Vec<FolderId>> {
        if learned.is_empty() && followed.is_empty() {
            return Ok(Vec::new());
        }
        let os = Os::CURRENT;
        let tx = self.write()?;
        for (id, identity) in learned {
            if read_folder(&tx, *id).is_ok() {
                upsert_meta(&tx, &identity_key(*id), identity)?;
            }
        }
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
            tx.execute(
                "UPDATE OR IGNORE meta SET key = ?2 WHERE key = ?1",
                params![
                    format!("local.{AUTO_MEMORY_PREFIX}{}", f.old),
                    format!("local.{AUTO_MEMORY_PREFIX}{}", f.new)
                ],
            )?;
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
                changes.push((f.id, stored));
            }
        }
        let tx = self.write()?;
        let now = now_ms();
        for (id, stored) in &changes {
            tx.execute(
                "UPDATE folders SET path = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
                params![id.0, stored, now, author],
            )?;
            tx.execute("DELETE FROM meta WHERE key = ?1", [identity_key(*id)])?;
        }
        tx.commit()?;
        let _ = self.refresh_link_index();
        Ok(changes.into_iter().map(|(id, _)| id).collect())
    }

    /// The "also include inherited folders" switch of an epic or a topic.
    pub fn set_include_inherited(
        &mut self,
        owner: Owner,
        include: bool,
        author: Author,
    ) -> Result<()> {
        let (table, id) = match owner {
            Owner::Epic(id) => ("epics", self.epic(id)?.id.0),
            Owner::Topic(id) => ("topics", self.topic(id)?.id.0),
            Owner::Project(_) => {
                return Err(Error::Invalid("a project has no inherited folders".into()));
            }
        };
        let tx = self.write()?;
        tx.execute(
            &format!("UPDATE {table} SET include_inherited_folders = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1"),
            params![id, include, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Nearest wins: the owner's own folders if it has any (plus the
    /// parent's effective ones as additional folders when its switch is on),
    /// otherwise the parent's effective folders.
    pub fn effective_folders(&self, owner: Owner) -> Result<EffectiveFolders> {
        let (parent, include) = match owner {
            Owner::Project(id) => {
                self.project(id)?;
                (None, false)
            }
            Owner::Epic(id) => {
                let e = self.epic(id)?;
                (Some(Owner::Project(e.project)), e.include_inherited_folders)
            }
            Owner::Topic(id) => {
                let t = self.topic(id)?;
                let parent = t.epic.map_or(Owner::Project(t.project), Owner::Epic);
                (Some(parent), t.include_inherited_folders)
            }
        };
        let own: Vec<EffectiveFolder> = self
            .folders(owner)?
            .into_iter()
            .map(|f| EffectiveFolder {
                path: f.path,
                from: owner,
            })
            .collect();
        let Some(parent) = parent else {
            return Ok(EffectiveFolders {
                folders: own,
                inherited_from: None,
            });
        };
        if own.is_empty() {
            let inherited = self.effective_folders(parent)?;
            let from = inherited.inherited_from.or(Some(parent));
            return Ok(EffectiveFolders {
                inherited_from: if inherited.folders.is_empty() {
                    None
                } else {
                    from
                },
                folders: inherited.folders,
            });
        }
        let mut folders = own;
        if include {
            for f in self.effective_folders(parent)?.folders {
                if !folders
                    .iter()
                    .any(|o| folder_path::same(Os::CURRENT, &o.path, &f.path))
                {
                    folders.push(f);
                }
            }
        }
        Ok(EffectiveFolders {
            folders,
            inherited_from: None,
        })
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
