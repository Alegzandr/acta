//! `acta link` (ADR 0005 "Binding", ADR 0008): attaches a folder to a
//! project, an epic or a topic, and turns Claude Code's auto memory off in
//! that folder so Acta is the one memory (ADR 0005, section 1).

use acta_core::agent::git;
use acta_core::claude_settings::{self, LOCAL_SETTINGS, Restore, Wanted};
use acta_core::folder_path;
use acta_core::model::{Author, EpicId, Owner, ProjectId, Record, TopicId};
use acta_core::paths::Os;
use acta_core::store::Store;
use std::path::{Path, PathBuf};

pub struct LinkArgs {
    pub path: Option<PathBuf>,
    pub project: Option<String>,
    pub epic: Option<String>,
    pub topic: Option<String>,
    /// Append as an additional folder instead of making it the primary one.
    pub add: bool,
    pub remove: bool,
    /// Leave Claude Code's auto memory on in this folder.
    pub keep_auto_memory: bool,
}

/// The default folder: the worktree root when inside git, else the
/// current directory.
fn default_path(cwd: &Path) -> PathBuf {
    git::inspect(cwd).map_or_else(|| cwd.to_path_buf(), |i| i.root)
}

fn owner_of(
    store: &mut Store,
    a: &LinkArgs,
    folder: &Path,
    author: Author,
) -> Result<Owner, String> {
    let given = [&a.topic, &a.epic, &a.project]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if given > 1 {
        return Err("pass one of --topic, --epic or --project".into());
    }
    if let Some(t) = &a.topic {
        let id = TopicId::parse(t).ok_or("--topic takes a topic id such as T-42")?;
        store.topic(id).map_err(|e| e.to_string())?;
        return Ok(Owner::Topic(id));
    }
    if let Some(e) = &a.epic {
        let id = EpicId::parse(e).ok_or("--epic takes an epic id such as E-3")?;
        store.epic(id).map_err(|e| e.to_string())?;
        return Ok(Owner::Epic(id));
    }
    if let Some(p) = &a.project {
        let id = ProjectId::parse(p).ok_or("--project takes a project id such as P-1")?;
        store.project(id).map_err(|e| e.to_string())?;
        return Ok(Owner::Project(id));
    }
    let projects = store.projects().map_err(|e| e.to_string())?;
    match projects.as_slice() {
        [] => {
            let name = folder
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Project".into());
            let id = store
                .create_project(&name, "", author)
                .map_err(|e| e.to_string())?;
            Ok(Owner::Project(id))
        }
        [only] => Ok(Owner::Project(only.id)),
        many => {
            let list: Vec<String> = many
                .iter()
                .take(10)
                .map(|p| format!("{} {}", p.id, p.title))
                .collect();
            Err(format!(
                "several projects ({}); pass --project",
                list.join(", ")
            ))
        }
    }
}

pub fn run(store: &mut Store, a: LinkArgs, cwd: &Path, author: Author) -> Result<String, String> {
    let path = a.path.clone().unwrap_or_else(|| default_path(cwd));
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    let stored =
        folder_path::canonical(&path).map_err(|e| format!("cannot use this folder: {e}"))?;
    if a.remove {
        return unlink(store, &a, &stored);
    }
    let owner = owner_of(store, &a, Path::new(&stored), author)?;
    let existing = store.folders(owner).map_err(|e| e.to_string())?;
    let already = existing
        .iter()
        .find(|f| folder_path::same(Os::CURRENT, &f.path, &stored))
        .map(|f| f.id);
    let id = match already {
        Some(id) => id,
        None => store
            .add_folder(owner, Path::new(&stored), author)
            .map_err(|e| e.to_string())?,
    };
    if !a.add {
        store
            .reorder(Record::Folder(id), 0, author)
            .map_err(|e| e.to_string())?;
    }
    let role = if a.add { "additional" } else { "primary" };
    let mut msg = if already.is_some() {
        format!("ok {stored} already linked to {owner} ({role})")
    } else {
        format!("ok linked {stored} to {owner} ({role})")
    };
    if !a.keep_auto_memory {
        match auto_memory_off(store, Path::new(&stored)) {
            Ok(true) => msg.push_str(&format!(
                "\nClaude Code's auto memory is now off in this folder ({LOCAL_SETTINGS}), so durable memory goes to Acta. Undo: acta link --remove, or --keep-auto-memory."
            )),
            Ok(false) => {}
            Err(e) => msg.push_str(&format!("\nauto memory left as is: {e}")),
        }
    }
    Ok(msg)
}

fn unlink(store: &mut Store, a: &LinkArgs, stored: &str) -> Result<String, String> {
    let mut removed = Vec::new();
    for f in store.all_folders().map_err(|e| e.to_string())? {
        if f.stamp.deleted_at.is_some() || !folder_path::same(Os::CURRENT, &f.path, stored) {
            continue;
        }
        let wanted = match (&a.topic, &a.epic, &a.project) {
            (Some(t), _, _) => TopicId::parse(t).map(Owner::Topic) == Some(f.owner),
            (_, Some(e), _) => EpicId::parse(e).map(Owner::Epic) == Some(f.owner),
            (_, _, Some(p)) => ProjectId::parse(p).map(Owner::Project) == Some(f.owner),
            _ => true,
        };
        if wanted {
            store
                .delete(Record::Folder(f.id), Author::Human)
                .map_err(|e| e.to_string())?;
            removed.push(f.owner.to_string());
        }
    }
    let mut msg = if removed.is_empty() {
        format!("{stored} is not linked")
    } else {
        format!("ok unlinked {stored} from {}", removed.join(", "))
    };
    let still = store
        .all_folders()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|f| f.stamp.deleted_at.is_none() && folder_path::same(Os::CURRENT, &f.path, stored));
    if !still
        && claude_settings::remove(store, Path::new(stored)).map_err(|e| e.to_string())?
            == Restore::Restored
    {
        msg.push_str(&format!(
            "\nActa's entries are out of {LOCAL_SETTINGS}: auto memory back on, commits manual."
        ));
    }
    Ok(msg)
}

/// `acta link --attached [--project P-n]`: the folders already attached in
/// Acta (of one project, or of all), read from the store so no path goes
/// on a command line. Each gets what `acta link` gives a folder: Claude
/// Code's auto memory off (unless `keep_auto_memory`) and the hooks'
/// pre-check file brought up to date. Nothing is attached or reordered.
pub fn run_attached(
    store: &mut Store,
    project: Option<&str>,
    keep_auto_memory: bool,
) -> Result<String, String> {
    let project = match project {
        Some(p) => {
            let id = ProjectId::parse(p).ok_or("--project takes a project id such as P-1")?;
            store.project(id).map_err(|e| e.to_string())?;
            Some(id)
        }
        None => None,
    };
    let folders: Vec<String> = store
        .all_folders()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|f| f.stamp.deleted_at.is_none())
        .filter(|f| project.is_none_or(|p| f.project == p))
        .map(|f| f.path)
        .collect();
    let mut seen: Vec<String> = Vec::new();
    let (mut off, mut missing, mut failed) = (0, 0, 0);
    for path in folders {
        if seen
            .iter()
            .any(|s| folder_path::same(Os::CURRENT, s, &path))
        {
            continue;
        }
        seen.push(path.clone());
        if !Path::new(&path).is_dir() {
            missing += 1;
            continue;
        }
        if keep_auto_memory {
            continue;
        }
        match auto_memory_off(store, Path::new(&path)) {
            Ok(true) => off += 1,
            Ok(false) => {}
            Err(_) => failed += 1,
        }
    }
    let index = store.refresh_link_index().is_ok();
    let mut msg = format!("ok {} attached folder(s)", seen.len());
    if off > 0 {
        msg.push_str(&format!(" \u{b7} auto memory turned off in {off}"));
    }
    if missing > 0 {
        msg.push_str(&format!(" \u{b7} {missing} not on this computer"));
    }
    if failed > 0 {
        msg.push_str(&format!(
            " \u{b7} {failed} left as is (settings not readable)"
        ));
    }
    if !index {
        msg.push_str("\nwarning: the hooks' pre-check file could not be updated");
    }
    Ok(msg)
}

/// `acta setup --remove`: takes Acta's entries out of every folder's
/// Claude Code settings (auto memory back on, commit rules out; only what
/// Acta wrote). Returns the folders restored and those left as they were.
pub fn restore_all_auto_memory(store: &mut Store) -> Result<(usize, usize), String> {
    claude_settings::remove_all(store).map_err(|e| e.to_string())
}

/// Writes `"autoMemoryEnabled": false` into the folder's local settings
/// through the one settings writer (ledger, never clobber). Returns whether
/// it changed anything.
fn auto_memory_off(store: &mut Store, folder: &Path) -> Result<bool, String> {
    let wanted = Wanted {
        auto_memory_off: Some(true),
        commit: None,
    };
    let o = claude_settings::apply(store, folder, wanted)?;
    if o.tracked {
        return Err(format!("{LOCAL_SETTINGS} is tracked by git"));
    }
    Ok(o.added.iter().any(|a| a == "autoMemoryEnabled"))
}
