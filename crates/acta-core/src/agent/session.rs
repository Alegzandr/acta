//! What an agent session is bound to (ADR 0005 "Binding", ADR 0008 "The
//! reverse direction"): the project and the focus topic or epic.
//!
//! Order of resolution: `ACTA_TOPIC` / `ACTA_EPIC` / `ACTA_PROJECT`; then the
//! focus last saved for this Claude Code session; then the focus last saved
//! for this worktree and branch; then the deepest attached folder containing
//! the launch directory (or, for a linked git worktree, its main checkout).

use super::git;
use crate::folder_path;
use crate::model::{Author, EpicId, Owner, ProjectId, TopicId};
use crate::paths::Os;
use crate::store::Store;
use std::path::{Path, PathBuf};

/// What the process knows about its launch.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    /// The launch directory (`CLAUDE_PROJECT_DIR`, the hook's `cwd`).
    pub dir: Option<PathBuf>,
    /// Claude Code's session id.
    pub session_id: Option<String>,
    /// `ACTA_PROJECT`, `ACTA_EPIC`, `ACTA_TOPIC` as given.
    pub project: Option<String>,
    pub epic: Option<String>,
    pub topic: Option<String>,
    /// `ACTA_LAUNCH`: the opaque token of the Acta terminal that started
    /// this session, so the app can find its terminal (session presence).
    pub token: Option<String>,
}

impl Launch {
    /// From the environment of an MCP server spawned by Claude Code.
    pub fn from_env() -> Launch {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Launch {
            dir: var("CLAUDE_PROJECT_DIR")
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok()),
            session_id: var("CLAUDE_CODE_SESSION_ID"),
            project: var("ACTA_PROJECT"),
            epic: var("ACTA_EPIC"),
            topic: var("ACTA_TOPIC"),
            token: var("ACTA_LAUNCH"),
        }
    }

    /// Whether the embedded terminal (or the user) pinned the binding.
    pub fn has_env_binding(&self) -> bool {
        self.project.is_some() || self.epic.is_some() || self.topic.is_some()
    }

    /// Whether a process of this session should open the database at all:
    /// pinned by `ACTA_*`, or launched in a linked folder according to the
    /// salted-hash pre-check file (ADR 0005, section 6). Reads that one
    /// file, never the keychain, so unrelated repositories stay cheap.
    pub fn worth_opening(&self, data_dir: &Path) -> bool {
        self.has_env_binding()
            || crate::link_index::LinkIndex::read(data_dir)
                .is_some_and(|index| index.matches(Os::CURRENT, &self.candidate_dirs()))
    }

    /// Candidate directories in stored form, for the hooks' pre-check: the
    /// launch directory and, inside a linked worktree, the main checkout.
    pub fn candidate_dirs(&self) -> Vec<String> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        let mut out = vec![stored(dir)];
        if let Some(info) = git::inspect(dir)
            && let Some(main) = info.main_root
        {
            let m = stored(&main);
            if !out.contains(&m) {
                out.push(m);
            }
        }
        out
    }
}

fn stored(dir: &Path) -> String {
    folder_path::canonical(dir)
        .unwrap_or_else(|_| folder_path::normalize(Os::CURRENT, &dir.to_string_lossy()))
}

/// Parses `T-42`, `E-7`, `P-1` into an owner.
pub fn parse_owner(text: &str) -> Option<Owner> {
    let t = text.trim();
    let (prefix, _) = t.split_once('-')?;
    match prefix.to_ascii_uppercase().as_str() {
        "T" => TopicId::parse(t).map(Owner::Topic),
        "E" => EpicId::parse(t).map(Owner::Epic),
        "P" => ProjectId::parse(t).map(Owner::Project),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub author: Author,
    pub project: Option<ProjectId>,
    /// A topic or an epic; never a project (no focus means the project).
    pub focus: Option<Owner>,
    pub session_id: Option<String>,
    /// `<worktree root>|<branch>`: where "last used here" is saved.
    pub worktree_key: Option<String>,
    /// Topics sharing the attached folder when the folder was ambiguous.
    pub candidates: Vec<TopicId>,
}

const SESSION_KEY: &str = "focus.session.";
const WORKTREE_KEY: &str = "focus.worktree.";

impl Session {
    /// A session bound to nothing: tools answer that the folder is not linked.
    pub fn unbound(author: Author) -> Session {
        Session {
            author,
            project: None,
            focus: None,
            session_id: None,
            worktree_key: None,
            candidates: Vec::new(),
        }
    }

    pub fn resolve(store: &Store, launch: &Launch, author: Author) -> Session {
        let mut s = Session::unbound(author);
        s.session_id = launch.session_id.clone();

        // 1. The environment.
        if let Some(t) = launch.topic.as_deref().and_then(TopicId::parse)
            && let Ok(topic) = store.topic(t)
        {
            s.project = Some(topic.project);
            s.focus = Some(Owner::Topic(t));
        } else if let Some(e) = launch.epic.as_deref().and_then(EpicId::parse)
            && let Ok(epic) = store.epic(e)
        {
            s.project = Some(epic.project);
            s.focus = Some(Owner::Epic(e));
        }
        if let Some(p) = launch.project.as_deref().and_then(ProjectId::parse)
            && store.project(p).is_ok()
            && s.project.is_none_or(|cur| cur == p)
        {
            s.project = Some(p);
        }

        // The folder, for the project and as the last focus fallback.
        let folder = launch.dir.as_deref().and_then(|dir| {
            store.resolve_folder(dir).ok().flatten().or_else(|| {
                let main = git::inspect(dir)?.main_root?;
                store.resolve_folder(&main).ok().flatten()
            })
        });
        if s.project.is_none() {
            s.project = folder.as_ref().map(|f| f.project);
        }
        s.worktree_key = launch
            .dir
            .as_deref()
            .map(|d| worktree_key(d, folder.as_ref().map(|f| f.folder.as_str())));

        // 2. and 3. Saved focus for this session, then this worktree.
        if s.focus.is_none() {
            let keys = [
                s.session_id.as_ref().map(|id| format!("{SESSION_KEY}{id}")),
                s.worktree_key
                    .as_ref()
                    .map(|k| format!("{WORKTREE_KEY}{k}")),
            ];
            for key in keys.into_iter().flatten() {
                if let Some(owner) = store
                    .local_value(&key)
                    .ok()
                    .flatten()
                    .and_then(|v| v.split_whitespace().next().and_then(parse_owner))
                    && let Some(project) = live_project(store, owner)
                    // A saved focus never binds an unbound session: only
                    // the environment and attached folders do.
                    && s.project == Some(project)
                    && !matches!(owner, Owner::Project(_))
                {
                    s.project = Some(project);
                    s.focus = Some(owner);
                    break;
                }
            }
        }

        // 4. The attached folder.
        if s.focus.is_none()
            && let Some(f) = &folder
            && s.project == Some(f.project)
        {
            s.focus = f.topic.map(Owner::Topic).or(f.epic.map(Owner::Epic));
            s.candidates = f.candidates.clone();
        }
        s
    }

    /// Makes `owner` the focus and saves it for this session and this
    /// worktree. A project clears the focus.
    pub fn set_focus(&mut self, store: &mut Store, owner: Owner) -> crate::Result<()> {
        let project =
            live_project(store, owner).ok_or_else(|| crate::Error::NotFound(owner.to_string()))?;
        self.project = Some(project);
        self.focus = match owner {
            Owner::Project(_) => None,
            other => Some(other),
        };
        self.candidates.clear();
        self.save_focus(store)
    }

    /// Claude Code started a new session id in the same process (`/clear`):
    /// the focus carries over and is saved under the new id.
    pub fn adopt_session_id(&mut self, store: &mut Store, id: &str) -> crate::Result<()> {
        if id.is_empty() || self.session_id.as_deref() == Some(id) {
            return Ok(());
        }
        self.session_id = Some(id.to_owned());
        if self.focus.is_some() {
            self.save_focus(store)?;
        }
        Ok(())
    }

    fn save_focus(&self, store: &mut Store) -> crate::Result<()> {
        let value = self.focus.map(|f| f.to_string());
        if let Some(id) = &self.session_id {
            // Session rows carry their time and are pruned after 30 days;
            // session ids are never reused.
            let now = crate::time::now_ms();
            let stamped = value.as_ref().map(|v| format!("{v} {now}"));
            store.set_local_value(&format!("{SESSION_KEY}{id}"), stamped.as_deref())?;
            prune_sessions(store, now)?;
        }
        if let Some(k) = &self.worktree_key {
            store.set_local_value(&format!("{WORKTREE_KEY}{k}"), value.as_deref())?;
        }
        Ok(())
    }
}

/// How long the focus saved for a Claude Code session is kept.
pub const SESSION_FOCUS_TTL_MS: i64 = 30 * crate::time::MS_PER_DAY;

/// Removes session focus rows older than [`SESSION_FOCUS_TTL_MS`] (and
/// rows without a time, written before times were kept).
fn prune_sessions(store: &mut Store, now: i64) -> crate::Result<()> {
    let old: Vec<String> = store
        .local_values(SESSION_KEY)?
        .into_iter()
        .filter(|(_, v)| {
            v.split_whitespace()
                .nth(1)
                .and_then(|t| t.parse::<i64>().ok())
                .is_none_or(|t| now - t > SESSION_FOCUS_TTL_MS)
        })
        .map(|(k, _)| k)
        .collect();
    for k in old {
        store.set_local_value(&k, None)?;
    }
    Ok(())
}

fn live_project(store: &Store, owner: Owner) -> Option<ProjectId> {
    match owner {
        Owner::Project(p) => store.project(p).ok().map(|p| p.id),
        Owner::Epic(e) => store.epic(e).ok().map(|e| e.project),
        Owner::Topic(t) => store.topic(t).ok().map(|t| t.project),
    }
}

/// `<worktree root>|<branch>`; outside git, the attached folder the
/// directory resolved to, else the directory itself.
fn worktree_key(dir: &Path, folder: Option<&str>) -> String {
    match git::inspect(dir) {
        Some(info) => format!(
            "{}|{}",
            stored(&info.root),
            info.branch.as_deref().unwrap_or("-")
        ),
        None => folder.map_or_else(|| stored(dir), str::to_owned),
    }
}
