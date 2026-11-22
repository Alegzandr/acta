//! The one writer of Claude Code's per-folder settings (ADR 0011, "What
//! Acta writes to Claude Code's settings"; ADR 0005, auto memory): Acta's
//! entries in a folder's `.claude/settings.local.json`.
//!
//! Never clobber: the file is parsed (refused if it is not a JSON object),
//! only what is absent is added, a key, rule or variable the user set is
//! never changed, the file is written atomically, and a ledger (a local
//! value per folder, never exported) records exactly what Acta added and
//! whether it created the file. Changing what Acta wants takes out only
//! ledger entries still holding Acta's value; the file is deleted only if
//! Acta created it and it is left empty. A file Acta creates is excluded in
//! the repository's `info/exclude`; a file tracked by git is never written.
//!
//! The ledger also keeps the folder's commit settings as last applied:
//! The store's folder columns (`commit_mode`, `commit_as_user`,
//! `commit_on_default`) are the source of a folder's commit settings; the
//! ledger records only what was written. [`set_commit`], [`sync_path`] and
//! [`sync_all`] bring the files in line with the columns.

use crate::agent::git;
use crate::folder_path;
use crate::git_cmd;
use crate::model::{Author, Folder, FolderId};
use crate::paths::Os;
use crate::store::{AUTO_MEMORY_PREFIX, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Local-value prefix of the ledger, followed by the folder's stored path.
/// The store moves it with its folder when the folder is followed.
pub const LEDGER_PREFIX: &str = "claudesettings.";
/// The file, relative to the folder.
pub const LOCAL_SETTINGS: &str = ".claude/settings.local.json";

const AUTO_MEMORY_KEY: &str = "autoMemoryEnabled";
const ATTRIBUTION_KEY: &str = "attribution";
const ALLOW_COMMIT: [&str; 2] = ["Bash(git add *)", "Bash(git commit *)"];
const ALLOW_PUSH: &str = "Bash(git push)";
const DENY: [&str; 4] = [
    "Bash(git * --no-verify*)",
    "Bash(git push *--force*)",
    "Bash(git push -f*)",
    "Bash(git push * -f*)",
];
const PUSH_ENV: [(&str, &str); 2] = [("GIT_TERMINAL_PROMPT", "0"), ("GCM_INTERACTIVE", "never")];

pub use crate::model::CommitMode;

/// Local value: the Settings default of "Commit in my name, without
/// Co-Authored-By Claude" (`"1"` on; off when absent, as on a new install).
pub const AS_USER_DEFAULT_KEY: &str = "commit.as_user_default";

impl CommitMode {
    pub fn automatic(self) -> bool {
        self != CommitMode::Manual
    }

    /// Manual < Commit < Push.
    fn rank(self) -> u8 {
        match self {
            CommitMode::Manual => 0,
            CommitMode::Commit => 1,
            CommitMode::Push => 2,
        }
    }
}

/// A folder's commit settings, as the store will hold them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSettings {
    pub mode: CommitMode,
    /// "Commit in my name, without Co-Authored-By Claude" (resolved: the
    /// Settings default already applied).
    #[serde(default)]
    pub as_user: bool,
    /// "Also on <default branch>".
    #[serde(default)]
    pub on_default: bool,
}

/// What Acta wants in a folder; `None` leaves that part as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    /// `autoMemoryEnabled: false` (ADR 0005).
    pub auto_memory_off: Option<bool>,
    pub commit: Option<CommitSettings>,
}

impl Wanted {
    /// Everything of Acta's out (unlink, setup --remove).
    pub const NOTHING: Wanted = Wanted {
        auto_memory_off: Some(false),
        commit: Some(CommitSettings {
            mode: CommitMode::Manual,
            as_user: false,
            on_default: false,
        }),
    };
}

/// What one write did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Entries Acta added, such as `autoMemoryEnabled`, `allow:Bash(git add *)`.
    pub added: Vec<String>,
    /// Entries of Acta's taken out.
    pub removed: Vec<String>,
    /// Acta's wishes the user's own values decide: `attribution`,
    /// `autoMemoryEnabled`, `env.GIT_TERMINAL_PROMPT`.
    pub user_kept: Vec<String>,
    /// The file is tracked by git: nothing was written ("Claude Code will
    /// ask before each commit").
    pub tracked: bool,
    /// The file was created but its line could not be added to the
    /// repository's `info/exclude`: it may show as an untracked file.
    pub exclude_failed: bool,
}

impl Outcome {
    pub fn changed(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Ledger {
    /// Acta created the file.
    #[serde(default)]
    created: bool,
    /// Acta created the `.claude` directory.
    #[serde(default)]
    dir_created: bool,
    /// Top-level keys Acta added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deny: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    env: Vec<String>,
    /// Containers Acta created: `permissions`, `permissions.allow`,
    /// `permissions.deny`, `env`; taken out again once empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    made: Vec<String>,
    /// Entries Acta added that the user took out of the file: not added
    /// again while they stay wanted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    user_removed: Vec<String>,
    /// Wanted state.
    #[serde(default)]
    auto_memory_off: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<CommitSettings>,
}

impl Ledger {
    fn holds_nothing(&self) -> bool {
        self.keys.is_empty()
            && self.allow.is_empty()
            && self.deny.is_empty()
            && self.env.is_empty()
            && self.made.is_empty()
    }

    fn wants_nothing(&self) -> bool {
        !self.auto_memory_off && self.commit.is_none_or(|c| !c.mode.automatic())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Key(&'static str, Value),
    Allow(&'static str),
    Deny(&'static str),
    Env(&'static str, &'static str),
}

fn attribution_value() -> Value {
    json!({"commit": "", "pr": "", "sessionUrl": false})
}

fn key_value(key: &str) -> Option<Value> {
    match key {
        AUTO_MEMORY_KEY => Some(json!(false)),
        ATTRIBUTION_KEY => Some(attribution_value()),
        _ => None,
    }
}

fn env_value(name: &str) -> Option<&'static str> {
    PUSH_ENV.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// Acta's entries for the wanted state, in writing order.
fn desired(auto_memory_off: bool, commit: Option<CommitSettings>) -> Vec<Entry> {
    let mut out = Vec::new();
    if auto_memory_off {
        out.push(Entry::Key(AUTO_MEMORY_KEY, json!(false)));
    }
    let Some(c) = commit.filter(|c| c.mode.automatic()) else {
        return out;
    };
    out.extend(ALLOW_COMMIT.iter().map(|r| Entry::Allow(r)));
    if c.mode == CommitMode::Push {
        out.push(Entry::Allow(ALLOW_PUSH));
    }
    out.extend(DENY.iter().map(|r| Entry::Deny(r)));
    if c.mode == CommitMode::Push {
        out.extend(PUSH_ENV.iter().map(|(n, v)| Entry::Env(n, v)));
    }
    if c.as_user {
        out.push(Entry::Key(ATTRIBUTION_KEY, attribution_value()));
    }
    out
}

fn ledger_key(folder: &Path) -> String {
    format!("{LEDGER_PREFIX}{}", folder.to_string_lossy())
}

fn read_ledger(store: &Store, folder: &Path) -> crate::Result<Option<Ledger>> {
    Ok(store
        .local_value(&ledger_key(folder))?
        .map(|t| serde_json::from_str(&t).unwrap_or_default()))
}

fn save_ledger(store: &mut Store, folder: &Path, ledger: &Ledger) -> crate::Result<()> {
    let key = ledger_key(folder);
    if ledger.holds_nothing() && ledger.wants_nothing() {
        store.set_local_value(&key, None)
    } else {
        let text = serde_json::to_string(ledger).expect("json");
        store.set_local_value(&key, Some(&text))
    }
}

/// Moves the `automemory.<path>` markers of earlier versions into the
/// ledger. Cheap when there is nothing to move; run by every entry point.
pub fn migrate_auto_memory_markers(store: &mut Store) -> crate::Result<usize> {
    let markers = store.local_values(AUTO_MEMORY_PREFIX)?;
    for (key, how) in &markers {
        let folder = PathBuf::from(&key[AUTO_MEMORY_PREFIX.len()..]);
        let mut ledger = read_ledger(store, &folder)?.unwrap_or_default();
        ledger.auto_memory_off = true;
        ledger.created |= how == "created";
        if !ledger.keys.iter().any(|k| k == AUTO_MEMORY_KEY) {
            ledger.keys.push(AUTO_MEMORY_KEY.to_owned());
        }
        save_ledger(store, &folder, &ledger)?;
        store.set_local_value(key, None)?;
    }
    Ok(markers.len())
}

/// The attached folder containing a directory, with its commit settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderCommit {
    /// The folder's stored path.
    pub path: String,
    /// Its display name: the name given in Acta, else the last segment.
    pub name: String,
    pub settings: CommitSettings,
}

/// Whether "Commit in my name" is on by default (Settings > Claude Code).
pub fn as_user_default(store: &Store) -> crate::Result<bool> {
    Ok(store.local_value(AS_USER_DEFAULT_KEY)?.as_deref() == Some("1"))
}

/// Sets the Settings default, then brings every folder that follows it in
/// line. Returns the folders written and those left as they were.
pub fn set_as_user_default(store: &mut Store, on: bool) -> crate::Result<(usize, usize)> {
    store.set_local_value(AS_USER_DEFAULT_KEY, on.then_some("1"))?;
    sync_all(store)
}

/// The settings the folders at one path give, from the store's columns.
/// One path attached at several levels has one settings file: the
/// strictest wins (the lowest mode; each option only when all have it).
fn settings_of(folders: &[Folder], as_user_default: bool) -> CommitSettings {
    let Some(mode) = folders
        .iter()
        .map(|f| f.commit_mode)
        .min_by_key(|m| m.rank())
    else {
        return CommitSettings::default();
    };
    if !mode.automatic() {
        return CommitSettings::default();
    }
    CommitSettings {
        mode,
        as_user: folders
            .iter()
            .all(|f| f.commit_as_user.unwrap_or(as_user_default)),
        on_default: folders.iter().all(|f| f.commit_on_default),
    }
}

/// The live folders attached at exactly `path`.
fn folders_at(store: &Store, path: &str) -> crate::Result<Vec<Folder>> {
    Ok(store
        .all_folders()?
        .into_iter()
        .filter(|f| f.stamp.deleted_at.is_none() && folder_path::same(Os::CURRENT, &f.path, path))
        .collect())
}

/// The commit settings the user chose for `dir` (stored form): those of
/// the deepest attached folder containing it, from the store's columns
/// only. What the session-start briefing tells the agent. `None` outside
/// every attached folder.
pub fn chosen_commit_settings_for(store: &Store, dir: &str) -> crate::Result<Option<FolderCommit>> {
    let all: Vec<Folder> = store
        .all_folders()?
        .into_iter()
        .filter(|f| f.stamp.deleted_at.is_none())
        .collect();
    let Some(best) = all
        .iter()
        .filter_map(|f| folder_path::contains(Os::CURRENT, &f.path, dir).map(|d| (d, f)))
        .max_by_key(|(d, _)| *d)
        .map(|(_, f)| f.path.clone())
    else {
        return Ok(None);
    };
    let here: Vec<Folder> = all
        .into_iter()
        .filter(|f| folder_path::same(Os::CURRENT, &f.path, &best))
        .collect();
    let name = here
        .iter()
        .map(|f| f.name.trim())
        .find(|n| !n.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            best.rsplit(['/', '\\'])
                .find(|s| !s.is_empty())
                .unwrap_or(&best)
                .to_owned()
        });
    let settings = settings_of(&here, as_user_default(store)?);
    Ok(Some(FolderCommit {
        path: best,
        name,
        settings,
    }))
}

/// The commit settings the `pre-git` guard enforces in `dir`: the user's
/// choice ([`chosen_commit_settings_for`]), made stricter by what a
/// settings file still holds. `None` outside every attached folder.
pub fn commit_settings_for(store: &Store, dir: &str) -> crate::Result<Option<FolderCommit>> {
    let Some(FolderCommit {
        path: best,
        name,
        mut settings,
    }) = chosen_commit_settings_for(store, dir)?
    else {
        return Ok(None);
    };
    // What the file still holds when it could not be brought in line (the
    // folder was offline): the stricter of the two is enforced, so a
    // switch to Manual still guards the rules left in the file.
    if let Some(recorded) = read_ledger(store, Path::new(&best))?.and_then(|l| l.commit)
        && recorded.mode.rank() > settings.mode.rank()
    {
        settings = CommitSettings {
            mode: recorded.mode,
            as_user: recorded.as_user,
            on_default: recorded.on_default && settings.on_default,
        };
    }
    Ok(Some(FolderCommit {
        path: best,
        name,
        settings,
    }))
}

/// Sets a folder's commit settings in the store (refused for Claude Code,
/// as the store refuses it), then writes its Claude Code settings.
pub fn set_commit(
    store: &mut Store,
    id: FolderId,
    mode: CommitMode,
    as_user: Option<bool>,
    on_default: bool,
    author: Author,
) -> Result<Outcome, String> {
    store
        .set_commit_settings(id, mode, as_user, on_default, author)
        .map_err(|e| e.to_string())?;
    let path = store.folder(id).map_err(|e| e.to_string())?.path;
    sync_path(store, Path::new(&path))
}

/// Brings one folder's Claude Code settings in line with the store's
/// columns. A folder not on this computer is left for when it is found.
pub fn sync_path(store: &mut Store, path: &Path) -> Result<Outcome, String> {
    let here = folders_at(store, &path.to_string_lossy()).map_err(|e| e.to_string())?;
    let default = as_user_default(store).map_err(|e| e.to_string())?;
    let settings = settings_of(&here, default);
    let recorded = read_ledger(store, path)
        .map_err(|e| e.to_string())?
        .and_then(|l| l.commit)
        .unwrap_or_default();
    if !path.is_dir() || recorded == settings {
        return Ok(Outcome::default());
    }
    apply(
        store,
        path,
        Wanted {
            auto_memory_off: None,
            commit: Some(settings),
        },
    )
}

/// Brings every folder in line with the columns (after an import, a
/// changed Settings default): folders whose recorded settings differ are
/// written, and a ledger left at a path no folder has any more is taken
/// out. Returns the folders written and those left as they were.
pub fn sync_all(store: &mut Store) -> crate::Result<(usize, usize)> {
    migrate_auto_memory_markers(store)?;
    let mut paths: Vec<String> = Vec::new();
    for f in store.all_folders()? {
        if f.stamp.deleted_at.is_none()
            && !paths
                .iter()
                .any(|p| folder_path::same(Os::CURRENT, p, &f.path))
        {
            paths.push(f.path);
        }
    }
    let (mut written, mut kept) = (0, 0);
    for p in &paths {
        match sync_path(store, Path::new(p)) {
            Ok(o) if o.changed() => written += 1,
            Ok(_) => {}
            Err(_) => kept += 1,
        }
    }
    let orphans: Vec<PathBuf> = store
        .local_values(LEDGER_PREFIX)?
        .into_iter()
        .map(|(k, _)| k[LEDGER_PREFIX.len()..].to_owned())
        .filter(|l| !paths.iter().any(|p| folder_path::same(Os::CURRENT, p, l)))
        .map(PathBuf::from)
        .collect();
    for o in orphans {
        match remove(store, &o)? {
            Restore::Restored => written += 1,
            Restore::Kept => kept += 1,
            Restore::NotNeeded => {}
        }
    }
    Ok((written, kept))
}

/// Brings the folder's local settings to what Acta wants. `folder` is the
/// folder's stored path. An `Err` is a short reason: nothing was written
/// and the ledger is unchanged.
pub fn apply(store: &mut Store, folder: &Path, wanted: Wanted) -> Result<Outcome, String> {
    migrate_auto_memory_markers(store).map_err(|e| e.to_string())?;
    let mut ledger = read_ledger(store, folder)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    if let Some(off) = wanted.auto_memory_off {
        ledger.auto_memory_off = off;
    }
    if let Some(c) = wanted.commit {
        ledger.commit = Some(c).filter(|c| c.mode.automatic());
    }
    let want = desired(ledger.auto_memory_off, ledger.commit);
    let mut outcome = Outcome::default();
    if want.is_empty() && ledger.holds_nothing() {
        save_ledger(store, folder, &ledger).map_err(|e| e.to_string())?;
        return Ok(outcome);
    }
    if !folder.is_dir() {
        return Err("the folder is not on this computer".into());
    }
    if tracked(folder)? {
        outcome.tracked = true;
        save_ledger(store, folder, &ledger).map_err(|e| e.to_string())?;
        return Ok(outcome);
    }

    let file = folder.join(LOCAL_SETTINGS);
    no_links(folder)?;
    let (mut obj, existed) = match fs::read_to_string(&file) {
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(Value::Object(o)) => (o, true),
            _ => return Err(format!("{LOCAL_SETTINGS} is not a JSON object")),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Map::new(), false),
        Err(e) => return Err(format!("cannot read {LOCAL_SETTINGS}: {}", e.kind())),
    };
    if !existed {
        // The user deleted the file: what Acta had there counts as taken out
        // by the user (`take_out`), and a new file would be Acta's again.
        ledger.created = false;
        ledger.made.clear();
    }
    let before = obj.clone();
    take_out(&mut obj, &mut ledger, &want, &mut outcome)?;
    put_in(&mut obj, &mut ledger, &want, &mut outcome)?;
    drop_empty_containers(&mut obj, &mut ledger);

    if obj != before || (!existed && !obj.is_empty()) {
        let claude_dir = file.parent().expect("parent");
        if obj.is_empty() && ledger.created {
            fs::remove_file(&file).map_err(|e| e.kind().to_string())?;
            ledger.created = false;
            if ledger.dir_created {
                // Only an empty directory goes.
                let _ = fs::remove_dir(claude_dir);
                ledger.dir_created = false;
            }
        } else {
            if !claude_dir.is_dir() {
                fs::create_dir_all(claude_dir).map_err(|e| e.kind().to_string())?;
                ledger.dir_created = true;
            }
            let text = serde_json::to_string_pretty(&Value::Object(obj)).expect("json") + "\n";
            // Checked again just before: nothing is written through a link.
            no_links(folder)?;
            write_in_place(&file, &text).map_err(|e| e.kind().to_string())?;
            if !existed {
                ledger.created = true;
                outcome.exclude_failed = !exclude_from_git(folder);
            }
        }
    }
    save_ledger(store, folder, &ledger).map_err(|e| e.to_string())?;
    Ok(outcome)
}

/// Refuses a `.claude` directory or settings file that is a link (symbolic
/// link, junction or other reparse point), or that resolves outside the
/// folder: a tracked `.claude -> ../other/.claude` would otherwise get
/// commit rules written into another repository.
fn no_links(folder: &Path) -> Result<(), String> {
    let dir = folder.join(".claude");
    for p in [&dir, &folder.join(LOCAL_SETTINGS)] {
        match fs::symlink_metadata(p) {
            Ok(m) if is_link(&m) => {
                return Err(format!("{LOCAL_SETTINGS} goes through a link; not written"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot read {LOCAL_SETTINGS}: {}", e.kind())),
        }
    }
    if dir.exists() {
        let inside = match (fs::canonicalize(&dir), fs::canonicalize(folder)) {
            (Ok(d), Ok(f)) => d.starts_with(&f),
            _ => false,
        };
        if !inside {
            return Err(format!(
                "{LOCAL_SETTINGS} is outside the folder; not written"
            ));
        }
    }
    Ok(())
}

fn is_link(m: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: links, junctions and the like.
        if m.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    m.file_type().is_symlink()
}

/// Whether the settings file is tracked by git. Outside a repository, or
/// where git sees none or is not installed, nothing can be tracked; any
/// other failure to answer writes nothing.
fn tracked(folder: &Path) -> Result<bool, String> {
    if git::inspect(folder).is_none() {
        return Ok(false);
    }
    match git_cmd::is_tracked(folder, LOCAL_SETTINGS) {
        Ok(t) => Ok(t),
        Err(git_cmd::GitError::NotARepository | git_cmd::GitError::NoGit) => Ok(false),
        Err(e) => Err(format!(
            "cannot tell whether {LOCAL_SETTINGS} is tracked: {e}"
        )),
    }
}

fn entry_label(e: &Entry) -> String {
    match e {
        Entry::Key(k, _) => (*k).to_owned(),
        Entry::Allow(r) => format!("allow:{r}"),
        Entry::Deny(r) => format!("deny:{r}"),
        Entry::Env(n, _) => format!("env.{n}"),
    }
}

/// Takes out ledger entries no longer wanted, where they still hold Acta's
/// value; a value the user changed is theirs and stays.
fn take_out(
    obj: &mut Map<String, Value>,
    ledger: &mut Ledger,
    want: &[Entry],
    outcome: &mut Outcome,
) -> Result<(), String> {
    let wanted_key = |k: &str| {
        want.iter()
            .any(|e| matches!(e, Entry::Key(w, _) if *w == k))
    };
    let mut gone = Vec::new();
    for k in std::mem::take(&mut ledger.keys) {
        let acta = key_value(&k);
        if wanted_key(&k) && obj.get(&k) == acta.as_ref() {
            ledger.keys.push(k);
        } else if wanted_key(&k) && obj.get(&k).is_none() {
            gone.push(k);
        } else if !wanted_key(&k) && obj.get(&k).is_some() && obj.get(&k) == acta.as_ref() {
            obj.remove(&k);
            outcome.removed.push(k);
        }
    }
    for (list, field) in [(0, "allow"), (1, "deny")] {
        let entries = std::mem::take(if list == 0 {
            &mut ledger.allow
        } else {
            &mut ledger.deny
        });
        let mut keep = Vec::new();
        for rule in entries {
            let still = want.iter().any(|e| match e {
                Entry::Allow(r) => list == 0 && *r == rule,
                Entry::Deny(r) => list == 1 && *r == rule,
                _ => false,
            });
            let arr = permission_list(obj, field)?;
            let present = arr.is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&rule)));
            if still && present {
                keep.push(rule);
            } else if still {
                gone.push(format!("{field}:{rule}"));
            } else if present {
                if let Some(a) = permission_list_mut(obj, field) {
                    a.retain(|v| v.as_str() != Some(&rule));
                }
                outcome.removed.push(format!("{field}:{rule}"));
            }
        }
        if list == 0 {
            ledger.allow = keep;
        } else {
            ledger.deny = keep;
        }
    }
    for name in std::mem::take(&mut ledger.env) {
        let still = want
            .iter()
            .any(|e| matches!(e, Entry::Env(n, _) if *n == name));
        let env = obj.get("env").and_then(Value::as_object);
        let cur = env.and_then(|e| e.get(&name)).and_then(Value::as_str);
        let acta = env_value(&name);
        if cur.is_none() && still {
            gone.push(format!("env.{name}"));
        }
        if cur.is_none() || cur != acta {
            continue;
        }
        if still {
            ledger.env.push(name);
        } else {
            if let Some(Value::Object(e)) = obj.get_mut("env") {
                e.remove(&name);
            }
            outcome.removed.push(format!("env.{name}"));
        }
    }
    // Remembered only while still wanted.
    let labels: Vec<String> = want.iter().map(entry_label).collect();
    ledger.user_removed.extend(gone);
    ledger.user_removed.retain(|l| labels.contains(l));
    ledger.user_removed.dedup();
    Ok(())
}

fn permission_list<'a>(
    obj: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a Vec<Value>>, String> {
    match obj.get("permissions") {
        None => Ok(None),
        Some(Value::Object(p)) => match p.get(field) {
            None => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(_) => Err(format!(
                "permissions.{field} in {LOCAL_SETTINGS} is not a list"
            )),
        },
        Some(_) => Err(format!("permissions in {LOCAL_SETTINGS} is not an object")),
    }
}

fn permission_list_mut<'a>(
    obj: &'a mut Map<String, Value>,
    field: &str,
) -> Option<&'a mut Vec<Value>> {
    obj.get_mut("permissions")?
        .as_object_mut()?
        .get_mut(field)?
        .as_array_mut()
}

/// Adds every wanted entry that is absent and records it.
fn put_in(
    obj: &mut Map<String, Value>,
    ledger: &mut Ledger,
    want: &[Entry],
    outcome: &mut Outcome,
) -> Result<(), String> {
    // Refuse a malformed container before changing anything.
    permission_list(obj, "allow")?;
    permission_list(obj, "deny")?;
    if obj.get("env").is_some_and(|e| !e.is_object()) {
        return Err(format!("env in {LOCAL_SETTINGS} is not an object"));
    }
    for e in want {
        if ledger.user_removed.contains(&entry_label(e)) {
            continue;
        }
        match e {
            Entry::Key(k, v) => {
                if ledger.keys.iter().any(|x| x == k) {
                    continue;
                }
                match obj.get(*k) {
                    None => {
                        obj.insert((*k).to_owned(), v.clone());
                        ledger.keys.push((*k).to_owned());
                        outcome.added.push(entry_label(e));
                    }
                    Some(cur) if cur != v => outcome.user_kept.push((*k).to_owned()),
                    Some(_) => {}
                }
            }
            Entry::Allow(r) | Entry::Deny(r) => {
                let (field, recorded) = match e {
                    Entry::Allow(_) => ("allow", &mut ledger.allow),
                    _ => ("deny", &mut ledger.deny),
                };
                if recorded.iter().any(|x| x == r) {
                    continue;
                }
                if permission_list(obj, field)?
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(r)))
                {
                    continue; // the user's own rule: same effect, not ours
                }
                if !obj.contains_key("permissions") {
                    obj.insert("permissions".into(), json!({}));
                    ledger.made.push("permissions".into());
                }
                let perms = obj["permissions"].as_object_mut().expect("checked");
                if !perms.contains_key(field) {
                    perms.insert(field.into(), json!([]));
                    ledger.made.push(format!("permissions.{field}"));
                }
                perms[field].as_array_mut().expect("checked").push(json!(r));
                recorded.push((*r).to_owned());
                outcome.added.push(entry_label(e));
            }
            Entry::Env(n, v) => {
                if ledger.env.iter().any(|x| x == n) {
                    continue;
                }
                if !obj.contains_key("env") {
                    obj.insert("env".into(), json!({}));
                    ledger.made.push("env".into());
                }
                let env = obj["env"].as_object_mut().expect("checked");
                match env.get(*n) {
                    None => {
                        env.insert((*n).to_owned(), json!(v));
                        ledger.env.push((*n).to_owned());
                        outcome.added.push(entry_label(e));
                    }
                    Some(cur) if cur.as_str() != Some(v) => {
                        outcome.user_kept.push(format!("env.{n}"));
                    }
                    Some(_) => {}
                }
            }
        }
    }
    Ok(())
}

/// Containers Acta created and that are now empty go; the others stay.
fn drop_empty_containers(obj: &mut Map<String, Value>, ledger: &mut Ledger) {
    let mut made = std::mem::take(&mut ledger.made);
    for field in ["allow", "deny"] {
        let name = format!("permissions.{field}");
        if !made.contains(&name) {
            continue;
        }
        let perms = obj.get_mut("permissions").and_then(Value::as_object_mut);
        match perms {
            Some(p) => match p.get(field) {
                Some(Value::Array(a)) if a.is_empty() => {
                    p.remove(field);
                    made.retain(|m| m != &name);
                }
                Some(_) => {}
                None => made.retain(|m| m != &name),
            },
            None => made.retain(|m| m != &name),
        }
    }
    for name in ["permissions", "env"] {
        if !made.iter().any(|m| m == name) {
            continue;
        }
        match obj.get(name) {
            Some(Value::Object(o)) if o.is_empty() => {
                obj.remove(name);
                made.retain(|m| m != name);
            }
            Some(_) => {}
            None => made.retain(|m| m != name),
        }
    }
    ledger.made = made;
}

/// Takes out everything of Acta's in one folder. `Kept`: not done (folder
/// not on this computer, file unreadable), the ledger stays to try again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restore {
    Restored,
    NotNeeded,
    Kept,
}

pub fn remove(store: &mut Store, folder: &Path) -> crate::Result<Restore> {
    migrate_auto_memory_markers(store)?;
    let Some(ledger) = read_ledger(store, folder)? else {
        return Ok(Restore::NotNeeded);
    };
    if !folder.is_dir() {
        return Ok(Restore::Kept);
    }
    Ok(match apply(store, folder, Wanted::NOTHING) {
        Ok(o) if o.changed() => Restore::Restored,
        Ok(_) if ledger.holds_nothing() || !folder.join(LOCAL_SETTINGS).exists() => {
            Restore::NotNeeded
        }
        Ok(o) if o.tracked => Restore::Kept,
        Ok(_) => Restore::NotNeeded,
        Err(_) => Restore::Kept,
    })
}

/// `acta setup --remove`: everything of Acta's out of every folder with a
/// ledger. Returns the folders restored and those left as they were.
pub fn remove_all(store: &mut Store) -> crate::Result<(usize, usize)> {
    migrate_auto_memory_markers(store)?;
    let folders: Vec<PathBuf> = store
        .local_values(LEDGER_PREFIX)?
        .into_iter()
        .map(|(k, _)| PathBuf::from(&k[LEDGER_PREFIX.len()..]))
        .collect();
    let (mut restored, mut kept) = (0, 0);
    for folder in folders {
        match remove(store, &folder)? {
            Restore::Restored => restored += 1,
            Restore::NotNeeded => {}
            Restore::Kept => kept += 1,
        }
    }
    Ok((restored, kept))
}

/// Keeps a settings file Acta created out of commits: a line in the
/// repository's local `info/exclude`, never in a tracked `.gitignore` or
/// the global excludes. Appended only, read as bytes. Paths are compared
/// in their canonical form (8.3 names, case, `\\?\\`). Returns whether the
/// line is there (or nothing is needed: not in a repository).
fn exclude_from_git(folder: &Path) -> bool {
    let Some(info) = git::inspect(folder) else {
        return true;
    };
    let (Ok(f), Ok(r)) = (
        folder_path::canonical(folder),
        folder_path::canonical(&info.root),
    ) else {
        return false;
    };
    if folder_path::contains(Os::CURRENT, &r, &f).is_none() {
        return false;
    }
    let rel = f.get(r.len()..).unwrap_or("").trim_matches(['/', '\\']);
    let mut line = rel.replace('\\', "/");
    if !line.is_empty() {
        line.push('/');
    }
    let line = format!("/{line}{LOCAL_SETTINGS}");
    let exclude = info.common_dir.join("info").join("exclude");
    let cur = match fs::read(&exclude) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return false,
    };
    if cur
        .split(|b| *b == b'\n')
        .any(|l| l.trim_ascii() == line.as_bytes())
    {
        return true;
    }
    let mut add = Vec::new();
    if !cur.is_empty() && !cur.ends_with(b"\n") {
        add.push(b'\n');
    }
    add.extend_from_slice(line.as_bytes());
    add.push(b'\n');
    let _ = fs::create_dir_all(exclude.parent().expect("parent"));
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&exclude)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(&add)
        })
        .is_ok()
}

/// Replaces a file atomically (a temporary file beside it, then a rename),
/// following a symbolic link and keeping the file's permissions. For the
/// user's own settings, which may be a dotfiles link.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    replace(&target, text)
}

/// [`write_atomic`] without following a link: a link at `path` is refused.
/// For a folder's settings, which a repository may point anywhere.
fn write_in_place(path: &Path, text: &str) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| is_link(&m)) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    replace(path, text)
}

fn replace(target: &Path, text: &str) -> io::Result<()> {
    let target = target.to_path_buf();
    let permissions = fs::metadata(&target).ok().map(|m| m.permissions());
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = target.with_file_name(format!(".{name}.acta-tmp-{}", std::process::id()));
    let result = fs::write(&tmp, text)
        .and_then(|()| match permissions {
            Some(p) => fs::set_permissions(&tmp, p),
            None => Ok(()),
        })
        .and_then(|()| fs::rename(&tmp, &target));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests;
