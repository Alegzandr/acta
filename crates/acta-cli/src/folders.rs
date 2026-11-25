//! `acta folders` (ADR 0011). `list` shows the folders a page sees;
//! `active` shows or sets the page's active folder, by its number or name,
//! never by a path; `commit` sets an attached folder's commit mode, written
//! to Claude Code's settings for that folder by the one settings writer,
//! and enforced by the `pre-git` guard. Setting is the user's only:
//! refused when Claude Code runs it.

use acta_core::agent::render::{Fence, Text};
use acta_core::agent::{Session, git, parse_owner};
use acta_core::claude_settings::{self, CommitMode, LOCAL_SETTINGS};
use acta_core::folder_path;
use acta_core::launch_plan::{self, LeftOut, PlanFolder};
use acta_core::model::{Author, Owner};
use acta_core::paths::Os;
use acta_core::store::{Store, VisibleFolders};
use std::path::{Path, PathBuf};

pub const REFUSED_FOR_CLAUDE: &str = "Commit modes are set by the user in Acta.";
pub const ACTIVE_REFUSED_FOR_CLAUDE: &str = "The active folder is set by the user in Acta.";

/// What `list` prints at most: a page sees few folders.
const LIST_BUDGET: usize = 4_000;

/// The page `id` names, else the session's focus, else its project.
fn page(store: &Store, session: &Session, id: Option<&str>) -> Result<Owner, String> {
    match id {
        Some(id) => {
            let o = parse_owner(id).ok_or("id must be T-n, E-n or P-n")?;
            let live = match o {
                Owner::Topic(t) => store.topic(t).is_ok_and(|t| t.stamp.deleted_at.is_none()),
                Owner::Epic(e) => store.epic(e).is_ok_and(|e| e.stamp.deleted_at.is_none()),
                Owner::Project(p) => store.project(p).is_ok_and(|p| p.stamp.deleted_at.is_none()),
            };
            if !live {
                return Err(format!("{o} not found"));
            }
            Ok(o)
        }
        None => session
            .focus
            .or(session.project.map(Owner::Project))
            .ok_or_else(|| "this folder is not linked to Acta; pass an id".to_owned()),
    }
}

fn mode_label(mode: CommitMode) -> &'static str {
    match mode {
        CommitMode::Manual => "manual commits",
        CommitMode::Commit => "commit automatically",
        CommitMode::Push => "commit and push automatically",
    }
}

fn plan(page: Owner, v: &VisibleFolders) -> launch_plan::LaunchPlan {
    launch_plan::launch_plan(
        &v.folders
            .iter()
            .map(|f| PlanFolder {
                path: PathBuf::from(&f.folder.path),
                own: f.origin == page,
                active: f.active,
            })
            .collect::<Vec<_>>(),
    )
}

/// `acta folders list [ID]`: numbered, the active one marked, with origin,
/// git state, commit mode and path (the user reads it; names are fenced).
pub fn list(store: &Store, session: &Session, id: Option<&str>) -> Result<String, String> {
    let page = page(store, session, id)?;
    let v = store.visible_folders(page).map_err(|e| e.to_string())?;
    if v.is_empty() {
        return Ok(format!(
            "{page} has no folders. Attach one in Acta or with acta link."
        ));
    }
    let plan = plan(page, &v);
    let mut out = Text::new(LIST_BUDGET);
    out.line(format!(
        "{page} sees {} folder{} (* active). The active folder and commit modes are set in Acta.",
        v.folders.len(),
        if v.folders.len() == 1 { "" } else { "s" }
    ));
    out.open_fence(&Fence::new());
    for (i, f) in v.folders.iter().enumerate() {
        let path = &f.folder.path;
        let mut parts = vec![format!(
            "{} {} {}",
            if f.active { "*" } else { " " },
            i + 1,
            f.folder.display_name()
        )];
        if f.origin != page {
            parts.push(format!("from {}", f.origin));
        }
        let left = plan
            .left_out
            .iter()
            .find(|(p, _)| folder_path::same(Os::CURRENT, &p.to_string_lossy(), path))
            .map(|(_, why)| *why);
        match left {
            Some(LeftOut::NotFound) => parts.push("not found".into()),
            Some(LeftOut::OtherCheckout) => {
                parts.push("not opened: another checkout of the same repository".into())
            }
            None => {}
        }
        if left != Some(LeftOut::NotFound)
            && let Some(info) = git::inspect(Path::new(path))
        {
            parts.push(match &info.branch {
                Some(b) => format!("git {b}"),
                None => "git, detached".into(),
            });
            // What this page uses: its own choice, a page above's, else the
            // folder's (ADR 0011 addendum 2026-11-23).
            if let Ok(e) = claude_settings::page::effective(store, page, path) {
                let c = e.settings;
                let mut m = mode_label(c.mode).to_owned();
                if c.mode.automatic() {
                    if c.as_user {
                        m.push_str(", in your name");
                    }
                    if c.on_default {
                        m.push_str(", also on the default branch");
                    }
                }
                if let claude_settings::page::Source::Page(o) = e.source {
                    m.push_str(&format!(" (set on {o})"));
                }
                parts.push(m);
            }
        }
        parts.push(path.clone());
        if !out.line(parts.join(" · ")) {
            break;
        }
    }
    Ok(out.finish())
}

/// `acta folders active [FOLDER] [--id ID]`: without FOLDER, the active
/// folder; with it (a number from `list`, a name, or `default`), makes it
/// the page's active folder. Never from Claude Code.
pub fn active(
    store: &mut Store,
    session: &Session,
    id: Option<&str>,
    folder: Option<&str>,
    author: Author,
) -> Result<String, String> {
    let page = page(store, session, id)?;
    let v = store.visible_folders(page).map_err(|e| e.to_string())?;
    let Some(want) = folder.map(str::trim) else {
        return Ok(match v.folders.iter().position(|f| f.active) {
            Some(i) => {
                let a = &v.folders[i].folder;
                let row = format!("{} {} · {}", i + 1, a.display_name(), a.path);
                format!("{page} active folder:\n{}", Fence::new().wrap(&row))
            }
            None => format!("{page} has no folders."),
        });
    };
    if author == Author::Claude {
        return Err(ACTIVE_REFUSED_FOR_CLAUDE.into());
    }
    let target = if want.eq_ignore_ascii_case("default") {
        None
    } else {
        let by_number = want
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| v.folders.get(i));
        let found = by_number.or_else(|| {
            let mut named = v
                .folders
                .iter()
                .filter(|f| f.folder.display_name().eq_ignore_ascii_case(want));
            match (named.next(), named.next()) {
                (Some(f), None) => Some(f),
                _ => None,
            }
        });
        Some(
            found
                .ok_or("FOLDER is a number from acta folders list, a name the page sees once, or default")?
                .folder
                .id,
        )
    };
    store
        .set_active_folder(page, target, author)
        .map_err(|e| e.to_string())?;
    let v = store.visible_folders(page).map_err(|e| e.to_string())?;
    let name = v
        .active()
        .map(|a| a.folder.display_name().to_owned())
        .unwrap_or_default();
    Ok(format!(
        "ok {page} active folder: {name}{}. A running session keeps its folder until restarted.",
        if target.is_none() { " (default)" } else { "" }
    ))
}

pub struct CommitArgs {
    pub mode: String,
    pub path: Option<PathBuf>,
    pub as_me: bool,
    pub on_default: bool,
}

pub fn commit(
    store: &mut Store,
    a: CommitArgs,
    cwd: &Path,
    author: Author,
) -> Result<String, String> {
    if author == Author::Claude {
        return Err(REFUSED_FOR_CLAUDE.into());
    }
    let mode = CommitMode::parse(&a.mode).ok_or("mode is manual, commit or push")?;
    let path = match a.path {
        Some(p) if p.is_absolute() => p,
        Some(p) => cwd.join(p),
        None => git::inspect(cwd).map_or_else(|| cwd.to_path_buf(), |i| i.root),
    };
    let stored =
        folder_path::canonical(&path).map_err(|e| format!("cannot use this folder: {e}"))?;
    let ids: Vec<_> = store
        .all_folders()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|f| {
            f.stamp.deleted_at.is_none() && folder_path::same(Os::CURRENT, &f.path, &stored)
        })
        .map(|f| f.id)
        .collect();
    if ids.is_empty() {
        return Err("this folder is not attached; run acta link first".into());
    }
    let info = git::inspect(Path::new(&stored));
    if mode.automatic() && info.is_none() {
        return Err("commit modes apply to folders in a git repository".into());
    }
    // The store's columns are the source: every level the folder is
    // attached at gets the same settings, then the file follows them.
    let as_user = a.as_me.then_some(true);
    let on_default = a.on_default && mode.automatic();
    for id in ids {
        store
            .set_commit_settings(id, mode, as_user, on_default, author)
            .map_err(|e| e.to_string())?;
    }
    let o = claude_settings::sync_path(store, Path::new(&stored))?;
    let settings = claude_settings::commit_settings_for(store, &stored)
        .map_err(|e| e.to_string())?
        .map(|f| f.settings)
        .unwrap_or_default();
    let mut msg = format!("ok {stored}: {}", mode_label(mode));
    if settings.as_user {
        msg.push_str(", in your name");
    }
    if mode.automatic() {
        let default = info.and_then(|i| i.default_branch);
        match (settings.on_default, default) {
            (true, Some(d)) => msg.push_str(&format!(", also on {d}")),
            (false, Some(d)) => msg.push_str(&format!(", not on {d}")),
            _ => {}
        }
    }
    if o.tracked {
        msg.push_str(&format!(
            "\n{LOCAL_SETTINGS} is tracked by git: Claude Code will ask before each commit."
        ));
    }
    if !o.user_kept.is_empty() {
        msg.push_str(&format!(
            "\nyour own settings decide: {}",
            o.user_kept.join(", ")
        ));
    }
    Ok(msg)
}
