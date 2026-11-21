//! `acta folders commit` (ADR 0011, section 3): sets an attached folder's
//! commit mode, written to Claude Code's settings for that folder by the
//! one settings writer, and enforced by the `pre-git` guard. Set by the
//! user only: refused when Claude Code runs it.

use acta_core::agent::git;
use acta_core::claude_settings::{self, CommitMode, CommitSettings, LOCAL_SETTINGS, Wanted};
use acta_core::folder_path;
use acta_core::model::Author;
use acta_core::paths::Os;
use acta_core::store::Store;
use std::path::{Path, PathBuf};

pub const REFUSED_FOR_CLAUDE: &str = "Commit modes are set by the user in Acta.";

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
    let attached = store
        .all_folders()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|f| f.stamp.deleted_at.is_none() && folder_path::same(Os::CURRENT, &f.path, &stored));
    if !attached {
        return Err("this folder is not attached; run acta link first".into());
    }
    let info = git::inspect(Path::new(&stored));
    if mode.automatic() && info.is_none() {
        return Err("commit modes apply to folders in a git repository".into());
    }
    let settings = CommitSettings {
        mode,
        as_user: a.as_me && mode.automatic(),
        on_default: a.on_default && mode.automatic(),
    };
    let wanted = Wanted {
        auto_memory_off: None,
        commit: Some(settings),
    };
    let o = claude_settings::apply(store, Path::new(&stored), wanted)?;
    let mut msg = format!(
        "ok {stored}: {}",
        match mode {
            CommitMode::Manual => "manual commits",
            CommitMode::Commit => "commit automatically",
            CommitMode::Push => "commit and push automatically",
        }
    );
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
