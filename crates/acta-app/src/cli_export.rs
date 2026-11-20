//! Exports the packaged CLI to a stable per-user directory (ADR 0004).
//!
//! Velopack kills every process running from its install root when it applies
//! an update, so `acta` (and a long-lived `acta mcp`) must live elsewhere. The
//! GUI copies `acta-cli` out at install, at update and at every start; a
//! version marker makes the repeat calls cheap.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use acta_core::paths::{self, BaseDirs, Os};

const MARKER: &str = "version.txt";
const ASIDE_TAG: &str = ".old-";
const INCOMING_TAG: &str = ".new-";

/// Everything one export needs, resolved up front so the work itself can be
/// pointed at a temporary directory in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The CLI inside the package, next to the GUI executable.
    pub source: PathBuf,
    /// Stable directory the CLI is exported to.
    pub dir: PathBuf,
    /// File name of the exported CLI (`acta` or `acta.exe`).
    pub exe_name: &'static str,
    /// Directory of the `acta` symlink; `None` on Windows.
    pub link_dir: Option<PathBuf>,
}

impl Layout {
    /// The layout for this process, or `None` when the executable or the
    /// user's directories cannot be determined.
    pub fn locate() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        Self::resolve(&exe, Os::CURRENT, &BaseDirs::from_env())
    }

    pub fn resolve(gui_exe: &Path, os: Os, base: &BaseDirs) -> Option<Self> {
        Some(Self {
            source: gui_exe.parent()?.join(paths::packaged_cli_name(os)),
            dir: paths::cli_export_dir(os, base)?,
            exe_name: paths::exported_cli_name(os),
            link_dir: paths::cli_link_dir(os, base),
        })
    }

    pub fn exported_exe(&self) -> PathBuf {
        self.dir.join(self.exe_name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The exported CLI is already this version.
    UpToDate,
    /// Missing, or left by another version: copy it.
    Export,
}

/// The whole idempotence rule: export unless the file is there and the marker
/// names exactly this version.
pub fn decide(marker: Option<&str>, exported_exists: bool, version: &str) -> Decision {
    if exported_exists && marker.map(str::trim) == Some(version) {
        Decision::UpToDate
    } else {
        Decision::Export
    }
}

/// Files this module leaves behind on purpose: a previous CLI that was renamed
/// aside while running, or a copy interrupted before its final rename.
pub fn is_leftover(file_name: &str) -> bool {
    file_name.contains(ASIDE_TAG) || file_name.contains(INCOMING_TAG)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub decision: Decision,
    /// Leftovers that could not be deleted yet (still running).
    pub still_locked: usize,
}

/// Bring the exported CLI to `version`. `stamp` only has to differ between
/// calls; it keeps the temporary names unique.
pub fn export(layout: &Layout, version: &str, stamp: u128) -> io::Result<Outcome> {
    fs::create_dir_all(&layout.dir)?;
    let still_locked = remove_leftovers(&layout.dir);

    let target = layout.exported_exe();
    let marker_path = layout.dir.join(MARKER);
    let marker = fs::read_to_string(&marker_path).ok();
    let decision = decide(marker.as_deref(), target.exists(), version);
    if decision == Decision::UpToDate {
        return Ok(Outcome {
            decision,
            still_locked,
        });
    }

    // Copy first, so a failed copy leaves the working CLI untouched.
    let incoming = layout
        .dir
        .join(format!("{}{INCOMING_TAG}{stamp}", layout.exe_name));
    fs::copy(&layout.source, &incoming)?;

    // A running executable cannot be overwritten on Windows, but it can be
    // renamed: move it aside, then put the new one in its place.
    let aside = layout
        .dir
        .join(format!("{}{ASIDE_TAG}{stamp}", layout.exe_name));
    let had_previous = target.exists();
    if had_previous && let Err(err) = fs::rename(&target, &aside) {
        let _ = fs::remove_file(&incoming);
        return Err(err);
    }
    if let Err(err) = fs::rename(&incoming, &target) {
        if had_previous {
            let _ = fs::rename(&aside, &target);
        }
        let _ = fs::remove_file(&incoming);
        return Err(err);
    }
    fs::write(&marker_path, version)?;

    // The file moved aside is deleted now if nothing runs it, else at a later start.
    let still_locked = remove_leftovers(&layout.dir);
    Ok(Outcome {
        decision,
        still_locked,
    })
}

/// Delete leftovers; returns how many are still in use.
fn remove_leftovers(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| is_leftover(&entry.file_name().to_string_lossy()))
        .filter(|entry| fs::remove_file(entry.path()).is_err())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(root: &Path) -> Layout {
        let package = root.join("package");
        fs::create_dir_all(&package).unwrap();
        Layout {
            source: package.join("acta-cli"),
            dir: root.join("export").join("bin"),
            exe_name: "acta",
            link_dir: None,
        }
    }

    #[test]
    fn decide_exports_when_missing_or_other_version() {
        assert_eq!(decide(None, false, "0.1.0"), Decision::Export);
        assert_eq!(decide(None, true, "0.1.0"), Decision::Export);
        assert_eq!(decide(Some("0.1.0"), false, "0.1.0"), Decision::Export);
        assert_eq!(decide(Some("0.0.9"), true, "0.1.0"), Decision::Export);
        // A downgrade re-exports too: the CLI always matches the GUI.
        assert_eq!(decide(Some("0.2.0"), true, "0.1.0"), Decision::Export);
    }

    #[test]
    fn decide_skips_only_on_exact_version_with_file_present() {
        assert_eq!(decide(Some("0.1.0"), true, "0.1.0"), Decision::UpToDate);
        assert_eq!(decide(Some("0.1.0\n"), true, "0.1.0"), Decision::UpToDate);
    }

    #[test]
    fn leftovers_are_recognised_and_real_files_are_not() {
        assert!(is_leftover("acta.exe.old-1763644800000"));
        assert!(is_leftover("acta.new-1763644800000"));
        assert!(!is_leftover("acta.exe"));
        assert!(!is_leftover("acta"));
        assert!(!is_leftover(MARKER));
    }

    #[test]
    fn resolve_uses_the_directory_of_the_gui_executable() {
        let base = BaseDirs {
            home: Some(PathBuf::from("home").join("alexandre")),
            local_app_data: Some(PathBuf::from("local")),
        };
        let exe = PathBuf::from("root").join("current").join("Acta.exe");
        let layout = Layout::resolve(&exe, Os::Windows, &base).unwrap();
        assert_eq!(
            layout.source,
            PathBuf::from("root").join("current").join("acta-cli.exe")
        );
        assert_eq!(
            layout.exported_exe(),
            PathBuf::from("local")
                .join("ActaCli")
                .join("bin")
                .join("acta.exe")
        );
        assert_eq!(layout.link_dir, None);

        let layout = Layout::resolve(&exe, Os::Linux, &base).unwrap();
        assert_eq!(layout.exe_name, "acta");
        assert!(layout.link_dir.is_some());
        assert_eq!(Layout::resolve(&exe, Os::Linux, &BaseDirs::default()), None);
    }

    #[test]
    fn first_export_copies_and_writes_the_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();

        let outcome = export(&layout, "0.1.0", 1).unwrap();

        assert_eq!(outcome.decision, Decision::Export);
        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.0");
        assert_eq!(
            fs::read_to_string(layout.dir.join(MARKER)).unwrap(),
            "0.1.0"
        );
    }

    #[test]
    fn same_version_is_a_no_op() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();

        // Even if the packaged file changed, the marker decides.
        fs::write(&layout.source, b"something else").unwrap();
        let outcome = export(&layout, "0.1.0", 2).unwrap();

        assert_eq!(outcome.decision, Decision::UpToDate);
        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.0");
    }

    #[test]
    fn new_version_replaces_and_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();

        fs::write(&layout.source, b"cli 0.1.1").unwrap();
        let outcome = export(&layout, "0.1.1", 2).unwrap();

        assert_eq!(outcome.decision, Decision::Export);
        assert_eq!(outcome.still_locked, 0);
        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.1");
        assert_eq!(
            fs::read_to_string(layout.dir.join(MARKER)).unwrap(),
            "0.1.1"
        );
        let names: Vec<String> = fs::read_dir(&layout.dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !is_leftover(n)), "{names:?}");
    }

    #[test]
    fn missing_source_fails_and_keeps_the_previous_cli() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();

        fs::remove_file(&layout.source).unwrap();
        assert!(export(&layout, "0.1.1", 2).is_err());

        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.0");
        assert_eq!(
            fs::read_to_string(layout.dir.join(MARKER)).unwrap(),
            "0.1.0"
        );
    }

    #[test]
    fn stale_leftovers_are_removed_on_the_next_call() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();
        let stale = layout.dir.join("acta.old-7");
        fs::write(&stale, b"old").unwrap();

        export(&layout, "0.1.0", 2).unwrap();

        assert!(!stale.exists());
    }
}
