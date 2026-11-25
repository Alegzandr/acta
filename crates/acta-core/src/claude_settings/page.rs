//! Commit mode per page (ADR 0011, addendum 2026-11-23): a project, epic or
//! topic may override the commit mode (and its two options) of any folder it
//! sees, inherited downward; without one it uses the folder's own setting.
//!
//! Overrides are local values (`pagecommit.<page>.<folder id>`): in the
//! encrypted database and never exported, so an import can never bring
//! commit settings in (ADR 0011, 4, as amended).
//!
//! A session Acta starts carries its page: [`begin_session`] writes a
//! per-session settings file under the data directory (permission rules,
//! `env` and `attribution` only: no user content) for `--settings`, and
//! records the launch under the session's `ACTA_LAUNCH` token. The guard
//! resolves the page from that record only ([`session_page`]): never from
//! `ACTA_TOPIC` / `ACTA_EPIC` or presence, which a session can change, so a
//! session cannot claim another page's laxer mode. A session started outside
//! Acta has no token and keeps the folder's setting; a token Acta does not
//! know gets the strictest answer.

use super::{
    ALLOW_COMMIT, ALLOW_PUSH, CommitMode, CommitSettings, DENY, Entry, FolderCommit,
    as_user_default, chosen_commit_settings_for, commit_settings_for, desired, folders_at,
    read_ledger, settings_of, stored_form,
};
use crate::error::{Error, Result};
use crate::folder_path;
use crate::model::{Author, EpicId, Folder, FolderId, Owner, ProjectId, TopicId};
use crate::paths::Os;
use crate::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Local-value prefix of a page's override, followed by `<page>.<folder id>`.
pub const PAGE_PREFIX: &str = "pagecommit.";
/// Local-value prefix of a session Acta started, followed by its token.
pub const LAUNCH_PREFIX: &str = "launch.";
/// The per-session settings files, in the data directory.
pub const SESSIONS_DIR: &str = "claude-sessions";

/// A page's own choice for one folder. `as_user: None` follows the
/// Settings default, as on a folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCommit {
    pub mode: CommitMode,
    #[serde(default)]
    pub as_user: Option<bool>,
    #[serde(default)]
    pub on_default: bool,
}

/// Where a page's commit settings for a folder come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The folder's own setting.
    Folder,
    /// The override of this page or of a level above it.
    Page(Owner),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effective {
    pub settings: CommitSettings,
    pub source: Source,
}

fn owner_key(owner: Owner) -> String {
    match owner {
        Owner::Project(id) => format!("p{}", id.0),
        Owner::Epic(id) => format!("e{}", id.0),
        Owner::Topic(id) => format!("t{}", id.0),
    }
}

fn parse_owner(s: &str) -> Option<Owner> {
    let (kind, id) = s.split_at_checked(1)?;
    let id: i64 = id.parse().ok()?;
    Some(match kind {
        "p" => Owner::Project(ProjectId(id)),
        "e" => Owner::Epic(EpicId(id)),
        "t" => Owner::Topic(TopicId(id)),
        _ => return None,
    })
}

fn page_key(owner: Owner, folder: FolderId) -> String {
    format!("{PAGE_PREFIX}{}.{}", owner_key(owner), folder.0)
}

/// The page's own override for a folder (not an inherited one).
pub fn page_commit(store: &Store, owner: Owner, folder: FolderId) -> Result<Option<PageCommit>> {
    Ok(store
        .local_value(&page_key(owner, folder))?
        .and_then(|t| serde_json::from_str(&t).ok()))
}

/// Sets (`Some`) or clears (`None`: use what the page inherits) a page's
/// override for a folder it sees. The user's only: refused for Claude Code.
/// Nothing is written to the folder's own settings file, which keeps only
/// what the folder's own setting allows.
pub fn set_page_commit(
    store: &mut Store,
    owner: Owner,
    folder: FolderId,
    value: Option<PageCommit>,
    author: Author,
) -> Result<()> {
    if author == Author::Claude {
        return Err(Error::Invalid(
            "Commit modes are set by the user in Acta.".into(),
        ));
    }
    let visible = store.visible_folders(owner)?;
    let Some(seen) = visible.folders.iter().find(|f| f.folder.id == folder) else {
        return Err(Error::Invalid(format!("{owner} does not see {folder}")));
    };
    // On its own folder a page sets the folder's setting itself.
    if seen.origin == owner {
        return Err(Error::Invalid(format!(
            "{folder} is {owner}'s own folder: set its commit mode"
        )));
    }
    let text = value.map(|v| serde_json::to_string(&v).expect("json"));
    store.set_local_value(&page_key(owner, folder), text.as_deref())
}

/// The overrides set at one level whose folder is live and seen by the
/// page, by folder path.
fn overrides_at(store: &Store, level: Owner, seen: &[Folder]) -> Result<Vec<(String, PageCommit)>> {
    let prefix = format!("{PAGE_PREFIX}{}.", owner_key(level));
    let mut out = Vec::new();
    for (key, text) in store.local_values(&prefix)? {
        let Ok(id) = key[prefix.len()..].parse::<i64>() else {
            continue;
        };
        let Ok(folder) = store.folder(FolderId(id)) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<PageCommit>(&text) else {
            continue;
        };
        if sees(seen, &folder.path) {
            out.push((folder.path, value));
        }
    }
    Ok(out)
}

fn resolve(value: PageCommit, as_user_default: bool) -> CommitSettings {
    if !value.mode.automatic() {
        return CommitSettings::default();
    }
    CommitSettings {
        mode: value.mode,
        as_user: value.as_user.unwrap_or(as_user_default),
        on_default: value.on_default,
    }
}

/// The settings `levels` (the page first) give the folder at `path`: the
/// nearest override, from `levels[skip]` on, set below the level that owns
/// the record the page sees; else that record's setting (one path may be
/// attached at several levels, each with its own setting); else the
/// folder's setting as its settings file has it.
fn effective_in(
    store: &Store,
    levels: &[Owner],
    skip: usize,
    seen: &[Folder],
    path: &str,
) -> Result<Effective> {
    let default = as_user_default(store)?;
    let record = seen
        .iter()
        .find(|f| folder_path::same(Os::CURRENT, &f.path, path));
    for (i, level) in levels.iter().enumerate() {
        // Only levels nearer than the one that owns the folder record the
        // page sees: that level sets it there, and a page's own record
        // ends the walk.
        if record.is_some_and(|f| f.owner == *level) {
            break;
        }
        if i < skip {
            continue;
        }
        if let Some((_, value)) = overrides_at(store, *level, seen)?
            .into_iter()
            .find(|(p, _)| folder_path::same(Os::CURRENT, p, path))
        {
            return Ok(Effective {
                settings: resolve(value, default),
                source: Source::Page(*level),
            });
        }
    }
    let settings = match record {
        Some(f) => settings_of(std::slice::from_ref(f), default),
        None => settings_of(&folders_at(store, path)?, default),
    };
    Ok(Effective {
        settings,
        source: Source::Folder,
    })
}

fn sees(seen: &[Folder], path: &str) -> bool {
    seen.iter()
        .any(|f| folder_path::same(Os::CURRENT, &f.path, path))
}

/// The folder records the page sees, nearest first.
fn seen_folders(store: &Store, owner: Owner) -> Result<Vec<Folder>> {
    Ok(store
        .visible_folders(owner)?
        .folders
        .into_iter()
        .map(|f| f.folder)
        .collect())
}

/// What a page uses for the folder at `path` (stored form): its own
/// override, else the nearest one above it, else the folder's setting. A
/// folder the page does not see always gives the folder's setting.
pub fn effective(store: &Store, owner: Owner, path: &str) -> Result<Effective> {
    let seen = seen_folders(store, owner)?;
    effective_in(store, &store.page_levels(owner)?, 0, &seen, path)
}

/// What the page would use without an override of its own: what "Use
/// folder setting" stands for in the folder form.
pub fn inherited(store: &Store, owner: Owner, path: &str) -> Result<Effective> {
    let seen = seen_folders(store, owner)?;
    let levels = store.page_levels(owner)?;
    effective_in(store, &levels, 1, &seen, path)
}

// ---- sessions Acta starts ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LaunchRecord {
    page: String,
    /// The working directory, stored form.
    folder: String,
    /// What the session's settings allow in it, as written at launch.
    granted: CommitSettings,
    /// The per-session settings file's name, if one was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// The app that started it: its records go when it is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app: Option<AppInstance>,
    /// Its folder was detached, removed or moved: the file was made strict.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    strict: bool,
}

/// An Acta app process: its id and start time (seconds since the epoch),
/// so a reused process id is not taken for the same app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInstance {
    pub pid: u32,
    pub started: u64,
}

/// The settings of a session whose folder no longer belongs to its page:
/// every add, commit and push asked, the automatic-mode denials kept.
fn strict_settings() -> Value {
    json!({"permissions": {
        "ask": [ALLOW_COMMIT[0], ALLOW_COMMIT[1], ALLOW_PUSH],
        "deny": DENY,
    }})
}

/// The per-session settings for a session whose page wants `wanted` in its
/// working folder, whose own settings file grants `file`: the rules of
/// `wanted`, and an `ask` rule for each thing the folder's file allows that
/// `wanted` does not (ask rules win over allow rules from any file). Empty
/// when there is nothing to say. Permission rules, `env` and `attribution`
/// only.
pub fn session_settings(wanted: CommitSettings, file: Option<CommitSettings>) -> Value {
    let mut allow = Vec::new();
    let mut deny = Vec::new();
    let mut env = Map::new();
    let mut doc = Map::new();
    for e in desired(false, Some(wanted)) {
        match e {
            Entry::Allow(r) => allow.push(json!(r)),
            Entry::Deny(r) => deny.push(json!(r)),
            Entry::Env(n, v) => {
                env.insert(n.to_owned(), json!(v));
            }
            Entry::Key(k, v) => {
                doc.insert(k.to_owned(), v);
            }
        }
    }
    let file = file.unwrap_or_default().mode;
    let mut ask = Vec::new();
    if file.automatic() && !wanted.mode.automatic() {
        ask.extend(ALLOW_COMMIT.iter().map(|r| json!(r)));
    }
    if file == CommitMode::Push && wanted.mode != CommitMode::Push {
        ask.push(json!(ALLOW_PUSH));
    }
    let mut permissions = Map::new();
    for (name, list) in [("allow", allow), ("deny", deny), ("ask", ask)] {
        if !list.is_empty() {
            permissions.insert(name.to_owned(), Value::Array(list));
        }
    }
    if !permissions.is_empty() {
        doc.insert("permissions".to_owned(), Value::Object(permissions));
    }
    if !env.is_empty() {
        doc.insert("env".to_owned(), Value::Object(env));
    }
    Value::Object(doc)
}

fn sessions_dir(store: &Store) -> PathBuf {
    store.data_dir().join(SESSIONS_DIR)
}

/// Records a session Acta is about to start in `folder` for `owner` under
/// its launch token, and writes its settings file. Returns the file to pass
/// with `--settings`, or `None` when the session needs none. The caller ends
/// it with [`end_session`].
pub fn begin_session(
    store: &mut Store,
    token: &str,
    owner: Owner,
    folder: &Path,
    app: Option<AppInstance>,
) -> Result<Option<PathBuf>> {
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(Error::Invalid("bad launch token".into()));
    }
    let path = stored_form(folder);
    let wanted = effective(store, owner, &path)?.settings;
    let file_grants = read_ledger(store, Path::new(&path))?.and_then(|l| l.commit);
    let doc = session_settings(wanted, file_grants);
    let mut file = None;
    if doc.as_object().is_some_and(|o| !o.is_empty()) {
        let dir = sessions_dir(store);
        fs::create_dir_all(&dir)?;
        let name = format!(
            "{}.json",
            crate::random_uuid().ok_or_else(|| Error::Invalid("no random source".into()))?
        );
        let target = dir.join(&name);
        fs::write(&target, serde_json::to_string_pretty(&doc).expect("json"))?;
        file = Some(name);
    }
    let record = LaunchRecord {
        page: owner_key(owner),
        folder: path,
        granted: wanted,
        file: file.clone(),
        app,
        strict: false,
    };
    store.set_local_value(
        &format!("{LAUNCH_PREFIX}{token}"),
        Some(&serde_json::to_string(&record).expect("json")),
    )?;
    Ok(file.map(|n| sessions_dir(store).join(n)))
}

/// Forgets a session Acta started and deletes its settings file.
pub fn end_session(store: &mut Store, token: &str) -> Result<()> {
    let key = format!("{LAUNCH_PREFIX}{token}");
    if let Some(record) = store
        .local_value(&key)?
        .and_then(|t| serde_json::from_str::<LaunchRecord>(&t).ok())
        && let Some(name) = record.file
    {
        let _ = fs::remove_file(sessions_dir(store).join(name));
    }
    store.set_local_value(&key, None)
}

fn records(store: &Store) -> Result<Vec<(String, LaunchRecord)>> {
    Ok(store
        .local_values(LAUNCH_PREFIX)?
        .into_iter()
        .filter_map(|(k, t)| Some((k, serde_json::from_str::<LaunchRecord>(&t).ok()?)))
        .collect())
}

/// Forgets the sessions whose app is gone (`alive` says whether an app
/// still runs), with their settings files, and settings files no record
/// names that are older than a minute. Run at the app's start: another
/// Acta instance's sessions are kept. Returns the records dropped.
pub fn end_sessions_of_gone_apps(
    store: &mut Store,
    alive: &dyn Fn(AppInstance) -> bool,
) -> Result<usize> {
    let mut dropped = 0;
    for (key, text) in store.local_values(LAUNCH_PREFIX)? {
        let record = serde_json::from_str::<LaunchRecord>(&text).ok();
        if record.as_ref().and_then(|r| r.app).is_some_and(alive) {
            continue;
        }
        if let Some(name) = record.and_then(|r| r.file) {
            let _ = fs::remove_file(sessions_dir(store).join(name));
        }
        store.set_local_value(&key, None)?;
        dropped += 1;
    }
    let named: Vec<String> = records(store)?
        .into_iter()
        .filter_map(|(_, r)| r.file)
        .collect();
    if let Ok(entries) = fs::read_dir(sessions_dir(store)) {
        let minute = std::time::Duration::from_secs(60);
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_none_or(|age| age > minute);
            if name.ends_with(".json") && !named.contains(&name) && old {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    Ok(dropped)
}

/// Whether the session's working folder still belongs to its page.
fn still_attached(store: &Store, r: &LaunchRecord) -> Result<bool> {
    let Some(page) = parse_owner(&r.page) else {
        return Ok(false);
    };
    match seen_folders(store, page) {
        Ok(seen) => Ok(sees(&seen, &r.folder)),
        Err(Error::NotFound(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Makes strict the settings file of every session whose working folder
/// was detached from its page, removed or moved (and records it), so its
/// file stops granting commits there. Returns the sessions made strict.
pub fn restrict_detached_sessions(store: &mut Store) -> Result<usize> {
    let mut n = 0;
    for (key, mut r) in records(store)? {
        if r.strict || still_attached(store, &r)? {
            continue;
        }
        if let Some(name) = &r.file {
            let text = serde_json::to_string_pretty(&strict_settings()).expect("json");
            let _ = super::write_atomic(&sessions_dir(store).join(name), &text);
        }
        r.strict = true;
        store.set_local_value(&key, Some(&serde_json::to_string(&r).expect("json")))?;
        n += 1;
    }
    Ok(n)
}

/// Whether the page's rules hold for a session Acta started: recorded, and
/// its folder still the page's.
pub fn session_rules_applied(store: &Store, token: &str) -> Result<bool> {
    Ok(store
        .local_value(&format!("{LAUNCH_PREFIX}{token}"))?
        .and_then(|t| serde_json::from_str::<LaunchRecord>(&t).ok())
        .is_some_and(|r| !r.strict))
}

/// A session Acta started, as recorded at launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launched {
    pub page: Owner,
    /// Its working directory, stored form.
    pub folder: String,
    pub granted: CommitSettings,
}

/// The page a hook's session works for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionPage {
    /// No launch token: started outside Acta. The folder's setting applies.
    Outside,
    Launched(Launched),
    /// A token Acta has no record of: the strictest answer.
    Unknown,
}

/// Resolves the session from its `ACTA_LAUNCH` token, and from nothing
/// else.
pub fn session_page(store: &Store, token: Option<&str>) -> Result<SessionPage> {
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return Ok(SessionPage::Outside);
    };
    let record = store
        .local_value(&format!("{LAUNCH_PREFIX}{token}"))?
        .and_then(|t| serde_json::from_str::<LaunchRecord>(&t).ok());
    Ok(
        match record.and_then(|r| Some((parse_owner(&r.page)?, r))) {
            Some((page, r)) => SessionPage::Launched(Launched {
                page,
                folder: r.folder,
                granted: r.granted,
            }),
            None => SessionPage::Unknown,
        },
    )
}

/// What the guard applies in a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardView {
    /// The attached folder containing the directory; its `settings` are
    /// what the commit and push checks apply.
    pub folder: FolderCommit,
    /// What the user wants there now: a commit or push this mode does not
    /// allow, but `granted` does, is put to the user.
    pub permitted: CommitMode,
    /// The most a settings file of the session may allow.
    pub granted: CommitMode,
}

fn laxer(a: CommitMode, b: CommitMode) -> CommitMode {
    if a.rank() >= b.rank() { a } else { b }
}

/// The guard's view of `dir` (stored form) for a session. `None` outside
/// every attached folder.
pub fn guard_view(store: &Store, dir: &str, session: &SessionPage) -> Result<Option<GuardView>> {
    match session {
        SessionPage::Outside => Ok(commit_settings_for(store, dir)?.map(|f| GuardView {
            permitted: f.settings.mode,
            granted: f.settings.mode,
            folder: f,
        })),
        SessionPage::Unknown => Ok(chosen_commit_settings_for(store, dir)?.map(|f| GuardView {
            folder: FolderCommit {
                settings: CommitSettings {
                    mode: CommitMode::Push,
                    as_user: false,
                    on_default: false,
                },
                ..f
            },
            permitted: CommitMode::Manual,
            granted: CommitMode::Push,
        })),
        SessionPage::Launched(l) => {
            let Some(here) = chosen_commit_settings_for(store, dir)? else {
                return detached(store, dir, l);
            };
            // The page's choice where it sees the folder; the folder's own
            // (including what its file still holds) where it does not.
            let seen = match seen_folders(store, l.page) {
                Ok(seen) => seen,
                // The page is gone (trashed): nothing tells what it wanted.
                Err(Error::NotFound(_)) => {
                    return guard_view(store, dir, &SessionPage::Unknown);
                }
                Err(e) => return Err(e),
            };
            let settings = if sees(&seen, &here.path) {
                effective_in(store, &store.page_levels(l.page)?, 0, &seen, &here.path)?.settings
            } else {
                commit_settings_for(store, dir)?.map_or(here.settings, |f| f.settings)
            };
            // The session's own file, and the working folder's file, which
            // Claude Code reads too, may allow more than the page wants now.
            let file = read_ledger(store, Path::new(&l.folder))?
                .and_then(|x| x.commit)
                .unwrap_or_default()
                .mode;
            let granted = laxer(laxer(l.granted.mode, file), settings.mode);
            Ok(Some(GuardView {
                permitted: settings.mode,
                granted,
                folder: FolderCommit { settings, ..here },
            }))
        }
    }
}

/// A directory in a session's working folder that is no longer attached
/// (detached, removed or moved): while a file of the session may still
/// allow commits there, each one is put to the user and the default branch
/// stays refused. Anywhere else, unattached as before.
fn detached(store: &Store, dir: &str, l: &Launched) -> Result<Option<GuardView>> {
    if folder_path::contains(Os::CURRENT, &l.folder, dir).is_none() {
        return Ok(None);
    }
    let file = read_ledger(store, Path::new(&l.folder))?
        .and_then(|x| x.commit)
        .unwrap_or_default()
        .mode;
    let granted = laxer(l.granted.mode, file);
    if !granted.automatic() {
        return Ok(None);
    }
    let name = l
        .folder
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(&l.folder)
        .to_owned();
    Ok(Some(GuardView {
        folder: FolderCommit {
            path: l.folder.clone(),
            name,
            settings: CommitSettings {
                mode: granted,
                as_user: false,
                on_default: false,
            },
        },
        permitted: CommitMode::Manual,
        granted,
    }))
}

/// What the session-start briefing tells the agent about commits in `dir`:
/// what the user chose for the session's page (never what a settings file
/// still holds). A session Acta does not know is told the strictest:
/// commits only when the user asks.
pub fn chosen_for_session(
    store: &Store,
    dir: &str,
    session: &SessionPage,
) -> Result<Option<FolderCommit>> {
    let Some(here) = chosen_commit_settings_for(store, dir)? else {
        return Ok(None);
    };
    let settings = match session {
        SessionPage::Outside => here.settings,
        SessionPage::Unknown => CommitSettings::default(),
        SessionPage::Launched(l) => match seen_folders(store, l.page) {
            Ok(seen) if sees(&seen, &here.path) => {
                effective_in(store, &store.page_levels(l.page)?, 0, &seen, &here.path)?.settings
            }
            Ok(_) => here.settings,
            Err(Error::NotFound(_)) => CommitSettings::default(),
            Err(e) => return Err(e),
        },
    };
    Ok(Some(FolderCommit { settings, ..here }))
}
