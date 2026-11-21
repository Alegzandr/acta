//! `acta link` (ADR 0005 "Binding", ADR 0008): attaches a folder to a
//! project, an epic or a topic, and turns Claude Code's auto memory off in
//! that folder so Acta is the one memory (ADR 0005, section 1).

use crate::setup::write_atomic;
use acta_core::agent::git;
use acta_core::folder_path;
use acta_core::model::{Author, EpicId, Owner, ProjectId, Record, TopicId};
use acta_core::paths::Os;
use acta_core::store::Store;
use serde_json::{Value, json};
use std::fs;
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

const AUTO_MEMORY_KEY: &str = "autoMemoryEnabled";
const LOCAL_SETTINGS: &str = ".claude/settings.local.json";

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
        && auto_memory_restore(store, Path::new(stored)).map_err(|e| e.to_string())?
            == Restore::Restored
    {
        msg.push_str("\nClaude Code's auto memory is back on in this folder.");
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

/// `acta setup --remove`: turns Claude Code's auto memory back on in every
/// folder where Acta turned it off (only what Acta wrote). Returns the
/// folders restored and those left as they were.
pub fn restore_all_auto_memory(store: &mut Store) -> Result<(usize, usize), String> {
    let folders: Vec<String> = store
        .local_values(AUTO_MEMORY_PREFIX)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(k, _)| k[AUTO_MEMORY_PREFIX.len()..].to_owned())
        .collect();
    let (mut restored, mut kept) = (0, 0);
    for folder in folders {
        match auto_memory_restore(store, Path::new(&folder)) {
            Ok(Restore::Restored) => restored += 1,
            Ok(Restore::NotNeeded) => {}
            Ok(Restore::Kept) | Err(_) => kept += 1,
        }
    }
    Ok((restored, kept))
}

/// Local-value prefix of the folders where Acta turned auto memory off; the
/// store moves a marker with its folder when the folder is followed.
pub use acta_core::store::AUTO_MEMORY_PREFIX;

fn marker_key(folder: &Path) -> String {
    format!("{AUTO_MEMORY_PREFIX}{}", folder.to_string_lossy())
}

/// Writes `"autoMemoryEnabled": false` into the folder's local settings,
/// unless the user already set the key either way. Remembers that Acta did
/// it, so `--remove` takes out only what Acta wrote. Returns whether it
/// changed anything.
fn auto_memory_off(store: &mut Store, folder: &Path) -> Result<bool, String> {
    let file = folder.join(LOCAL_SETTINGS);
    let (mut v, created) = match fs::read_to_string(&file) {
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(v @ Value::Object(_)) => (v, false),
            _ => return Err(format!("{LOCAL_SETTINGS} is not a JSON object")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (json!({}), true),
        Err(e) => return Err(format!("cannot read {LOCAL_SETTINGS}: {}", e.kind())),
    };
    if v.get(AUTO_MEMORY_KEY).is_some() {
        return Ok(false);
    }
    v[AUTO_MEMORY_KEY] = json!(false);
    fs::create_dir_all(file.parent().expect("parent")).map_err(|e| e.kind().to_string())?;
    write_atomic(
        &file,
        &(serde_json::to_string_pretty(&v).expect("json") + "\n"),
    )
    .map_err(|e| e.kind().to_string())?;
    store
        .set_local_value(
            &marker_key(folder),
            Some(if created { "created" } else { "added" }),
        )
        .map_err(|e| e.to_string())?;
    if created {
        exclude_from_git(folder);
    }
    Ok(true)
}

/// Keeps a settings file Acta created out of commits: a line in the
/// repository's local `info/exclude`, never in a tracked `.gitignore`.
fn exclude_from_git(folder: &Path) {
    let Some(info) = git::inspect(folder) else {
        return;
    };
    let Ok(rel) = folder.strip_prefix(&info.root) else {
        return;
    };
    let mut line = rel.to_string_lossy().replace('\\', "/");
    if !line.is_empty() {
        line.push('/');
    }
    let line = format!("/{line}{LOCAL_SETTINGS}");
    let exclude = info.common_dir.join("info").join("exclude");
    // Read as bytes (the file may not be UTF-8). Any read error other than
    // "not there" leaves the file alone; the line is only ever appended.
    let cur = match fs::read(&exclude) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return,
    };
    if cur
        .split(|b| *b == b'\n')
        .any(|l| l.trim_ascii() == line.as_bytes())
    {
        return;
    }
    let mut add = Vec::new();
    if !cur.is_empty() && !cur.ends_with(b"\n") {
        add.push(b'\n');
    }
    add.extend_from_slice(line.as_bytes());
    add.push(b'\n');
    let _ = fs::create_dir_all(exclude.parent().expect("parent"));
    if let Ok(mut f) = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&exclude)
    {
        use std::io::Write;
        let _ = f.write_all(&add);
    }
}

/// What turning auto memory back on did in one folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restore {
    /// Acta's `autoMemoryEnabled: false` was taken out.
    Restored,
    /// Nothing of Acta's is left there (the file or the key is gone, or
    /// the user set the key since): the record is cleared.
    NotNeeded,
    /// Not done: the folder is not on this computer, or its settings file
    /// cannot be read or written. The record stays, to try again later.
    Kept,
}

fn auto_memory_restore(store: &mut Store, folder: &Path) -> acta_core::Result<Restore> {
    let key = marker_key(folder);
    let Some(how) = store.local_value(&key)? else {
        return Ok(Restore::NotNeeded);
    };
    if !folder.is_dir() {
        return Ok(Restore::Kept);
    }
    let file = folder.join(LOCAL_SETTINGS);
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            store.set_local_value(&key, None)?;
            return Ok(Restore::NotNeeded);
        }
        Err(_) => return Ok(Restore::Kept),
    };
    let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(&text) else {
        return Ok(Restore::Kept);
    };
    if obj.get(AUTO_MEMORY_KEY) != Some(&json!(false)) {
        store.set_local_value(&key, None)?;
        return Ok(Restore::NotNeeded);
    }
    obj.remove(AUTO_MEMORY_KEY);
    let written = if obj.is_empty() && how == "created" {
        fs::remove_file(&file)
    } else {
        write_atomic(
            &file,
            &(serde_json::to_string_pretty(&Value::Object(obj)).expect("json") + "\n"),
        )
    };
    if written.is_err() {
        return Ok(Restore::Kept);
    }
    store.set_local_value(&key, None)?;
    Ok(Restore::Restored)
}
