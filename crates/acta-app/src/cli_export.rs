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
    /// The directory that exists for the export alone and contains `dir`.
    /// Uninstall deletes it whole; it never holds anything else.
    pub removable_root: PathBuf,
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
            removable_root: paths::cli_root(os, base)?,
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

/// The whole idempotence rule: export unless the exported file is there, the
/// marker names exactly this version and the file is as long as the packaged
/// one. The length catches a copy that was cut short (power loss, full disk)
/// after its marker had been written by an earlier, complete export.
pub fn decide(
    marker: Option<&str>,
    exported_len: Option<u64>,
    source_len: Option<u64>,
    version: &str,
) -> Decision {
    let same_version = marker.map(str::trim) == Some(version);
    if same_version && exported_len.is_some() && exported_len == source_len {
        Decision::UpToDate
    } else {
        Decision::Export
    }
}

/// Files this module leaves behind on purpose: a previous CLI that was renamed
/// aside while running, or a copy interrupted before its final rename.
#[cfg(test)]
fn is_leftover(file_name: &str) -> bool {
    file_name.contains(ASIDE_TAG) || file_name.contains(INCOMING_TAG)
}

/// How long an incoming copy is left alone: another Acta process (an install
/// hook next to a starting app) may still be writing it.
const INCOMING_GRACE_MS: u128 = 60_000;

/// Whether a leftover may be deleted at `now_ms`. Files renamed aside always
/// (deleting fails by itself while they run); incoming copies only once they
/// are older than the grace period, going by the stamp in their name.
pub fn is_removable(file_name: &str, now_ms: u128) -> bool {
    if file_name.contains(ASIDE_TAG) {
        return true;
    }
    match file_name.rsplit_once(INCOMING_TAG) {
        Some((_, stamp)) => match stamp.parse::<u128>() {
            Ok(created) => now_ms.saturating_sub(created) > INCOMING_GRACE_MS,
            // Not a name this module produces: nobody is writing it.
            Err(_) => true,
        },
        None => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub decision: Decision,
    /// Leftovers that could not be deleted yet (still running).
    pub still_locked: usize,
}

fn len(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|meta| meta.len())
}

/// Write `bytes` to `path` and have them reach the disk.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    io::Write::write_all(&mut file, bytes)?;
    file.sync_all()
}

/// Bring the exported CLI to `version`. `now_ms` is the current time in
/// milliseconds; it also keeps the temporary names unique.
pub fn export(layout: &Layout, version: &str, now_ms: u128) -> io::Result<Outcome> {
    fs::create_dir_all(&layout.dir)?;
    let still_locked = remove_leftovers(&layout.dir, now_ms);

    let target = layout.exported_exe();
    let marker_path = layout.dir.join(MARKER);
    let marker = fs::read_to_string(&marker_path).ok();
    let decision = decide(
        marker.as_deref(),
        len(&target),
        len(&layout.source),
        version,
    );
    if decision == Decision::UpToDate {
        return Ok(Outcome {
            decision,
            still_locked,
        });
    }

    // Copy first, so a failed copy leaves the working CLI untouched, and
    // flush it: the marker written below must never describe a file whose
    // bytes are still only in a cache.
    let incoming = layout
        .dir
        .join(format!("{}{INCOMING_TAG}{now_ms}", layout.exe_name));
    let copied = fs::copy(&layout.source, &incoming)
        .and_then(|_| fs::OpenOptions::new().write(true).open(&incoming))
        .and_then(|file| file.sync_all());
    if let Err(err) = copied {
        let _ = fs::remove_file(&incoming);
        return Err(err);
    }

    // A running executable cannot be overwritten on Windows, but it can be
    // renamed: move it aside, then put the new one in its place.
    let aside = layout
        .dir
        .join(format!("{}{ASIDE_TAG}{now_ms}", layout.exe_name));
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
    write_synced(&marker_path, version.as_bytes())?;

    // The file moved aside is deleted now if nothing runs it, else at a later start.
    let still_locked = remove_leftovers(&layout.dir, now_ms);
    Ok(Outcome {
        decision,
        still_locked,
    })
}

/// Delete the leftovers that may be deleted; returns how many of those are
/// still in use.
fn remove_leftovers(dir: &Path, now_ms: u128) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| is_removable(&entry.file_name().to_string_lossy(), now_ms))
        .filter(|entry| fs::remove_file(entry.path()).is_err())
        .count()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn layout(root: &Path) -> Layout {
        let package = root.join("package");
        fs::create_dir_all(&package).unwrap();
        Layout {
            source: package.join("acta-cli"),
            dir: root.join("export").join("bin"),
            removable_root: root.join("export"),
            exe_name: "acta",
            link_dir: None,
        }
    }

    #[test]
    fn decide_exports_when_missing_or_other_version() {
        let len = Some(9);
        assert_eq!(decide(None, None, len, "0.1.0"), Decision::Export);
        assert_eq!(decide(None, len, len, "0.1.0"), Decision::Export);
        assert_eq!(decide(Some("0.1.0"), None, len, "0.1.0"), Decision::Export);
        assert_eq!(decide(Some("0.0.9"), len, len, "0.1.0"), Decision::Export);
        // A downgrade re-exports too: the CLI always matches the GUI.
        assert_eq!(decide(Some("0.2.0"), len, len, "0.1.0"), Decision::Export);
    }

    #[test]
    fn decide_exports_when_the_exported_file_is_not_the_packaged_length() {
        // Truncated copy under a marker from an earlier complete export.
        assert_eq!(
            decide(Some("0.1.0"), Some(4), Some(9), "0.1.0"),
            Decision::Export
        );
        assert_eq!(
            decide(Some("0.1.0"), Some(0), Some(9), "0.1.0"),
            Decision::Export
        );
        // Packaged CLI missing: not provably up to date (the copy then fails
        // and is logged, the exported CLI stays as it is).
        assert_eq!(
            decide(Some("0.1.0"), Some(9), None, "0.1.0"),
            Decision::Export
        );
    }

    #[test]
    fn decide_skips_only_on_exact_version_and_equal_length() {
        let len = Some(9);
        assert_eq!(decide(Some("0.1.0"), len, len, "0.1.0"), Decision::UpToDate);
        assert_eq!(
            decide(Some("0.1.0\n"), len, len, "0.1.0"),
            Decision::UpToDate
        );
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
    fn incoming_copies_are_only_removable_after_the_grace_period() {
        let now = 1_763_644_800_000u128;
        assert!(!is_removable(&format!("acta.new-{now}"), now));
        assert!(!is_removable(&format!("acta.new-{}", now - 59_000), now));
        assert!(is_removable(&format!("acta.new-{}", now - 61_000), now));
        // A clock that went backwards does not make a fresh copy removable.
        assert!(!is_removable(&format!("acta.new-{}", now + 5_000), now));
        // Renamed-aside files are always tried; so are malformed names.
        assert!(is_removable(&format!("acta.exe.old-{now}"), now));
        assert!(is_removable("acta.new-garbage", now));
        // And nothing else is ever touched.
        assert!(!is_removable("acta.exe", now));
        assert!(!is_removable(MARKER, now));
    }

    #[test]
    fn resolve_uses_the_directory_of_the_gui_executable() {
        let base = BaseDirs {
            home: Some(PathBuf::from("home").join("alexandre")),
            local_app_data: Some(PathBuf::from("local")),
            ..BaseDirs::default()
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
        assert_eq!(
            layout.removable_root,
            PathBuf::from("local").join("ActaCli")
        );
        assert_eq!(layout.link_dir, None);

        let layout = Layout::resolve(&exe, Os::Linux, &base).unwrap();
        assert_eq!(layout.exe_name, "acta");
        assert!(layout.link_dir.is_some());
        assert_eq!(Layout::resolve(&exe, Os::Linux, &BaseDirs::default()), None);
    }

    #[test]
    fn the_root_uninstall_deletes_holds_the_export_and_never_the_data() {
        let base = BaseDirs {
            home: Some(PathBuf::from("home").join("alexandre")),
            local_app_data: Some(PathBuf::from("local")),
            ..BaseDirs::default()
        };
        let exe = PathBuf::from("root").join("current").join("Acta");
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let layout = Layout::resolve(&exe, os, &base).unwrap();
            let data = paths::data_dir(os, &base).unwrap();
            assert!(layout.dir.starts_with(&layout.removable_root), "{os:?}");
            assert_ne!(layout.dir, layout.removable_root, "{os:?}");
            assert_ne!(layout.removable_root, data, "{os:?}");
            assert!(!data.starts_with(&layout.removable_root), "{os:?}");
            assert!(!layout.source.starts_with(&layout.removable_root), "{os:?}");
        }
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
    fn same_version_and_length_is_a_no_op() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();

        // Same length, other bytes: marker and length decide, not content.
        fs::write(&layout.source, b"CLI 0.1.0").unwrap();
        let outcome = export(&layout, "0.1.0", 2).unwrap();

        assert_eq!(outcome.decision, Decision::UpToDate);
        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.0");
    }

    #[test]
    fn a_truncated_export_is_repaired_even_under_a_matching_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        export(&layout, "0.1.0", 1).unwrap();
        fs::write(layout.exported_exe(), b"cli").unwrap();

        let outcome = export(&layout, "0.1.0", 2).unwrap();

        assert_eq!(outcome.decision, Decision::Export);
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
    fn stale_leftovers_are_removed_and_a_fresh_incoming_copy_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = layout(tmp.path());
        fs::write(&layout.source, b"cli 0.1.0").unwrap();
        let now = 1_763_644_800_000u128;
        export(&layout, "0.1.0", now).unwrap();
        let aside = layout.dir.join("acta.old-7");
        let abandoned = layout.dir.join(format!("acta.new-{}", now - 120_000));
        let in_progress = layout.dir.join(format!("acta.new-{}", now - 1_000));
        for file in [&aside, &abandoned, &in_progress] {
            fs::write(file, b"x").unwrap();
        }

        export(&layout, "0.1.0", now).unwrap();

        assert!(!aside.exists());
        assert!(!abandoned.exists());
        assert!(in_progress.exists(), "another process may be writing it");
    }

    /// A long-lived process whose image is the exported CLI (Windows): a copy
    /// of the command interpreter waiting on a pipe that stays open.
    #[cfg(windows)]
    pub(crate) mod running {
        use std::fs;
        use std::process::{Child, Command, Stdio};

        use super::super::Layout;

        /// Kills the process when dropped, also when a test fails.
        pub(crate) struct Running(pub(crate) Child);

        impl Drop for Running {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        pub(crate) fn run_exported_cli(layout: &Layout) -> Running {
            let comspec = std::env::var_os("ComSpec").expect("ComSpec is set on Windows");
            fs::create_dir_all(&layout.dir).unwrap();
            fs::copy(comspec, layout.exported_exe()).unwrap();
            let child = Command::new(layout.exported_exe())
                .args(["/d", "/c", "pause"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            Running(child)
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_running_cli_is_renamed_aside_and_cleaned_up_once_it_exits() {
        let tmp = tempfile::tempdir().unwrap();
        let mut layout = layout(tmp.path());
        layout.exe_name = "acta.exe";
        let leftovers = |layout: &Layout| -> Vec<String> {
            fs::read_dir(&layout.dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| is_leftover(name))
                .collect()
        };
        // "0.1.0" is running from the export directory.
        let mut cli = running::run_exported_cli(&layout);
        fs::write(layout.dir.join(MARKER), "0.1.0").unwrap();
        assert!(cli.0.try_wait().unwrap().is_none(), "the CLI is running");
        assert!(
            fs::write(layout.exported_exe(), b"x").is_err(),
            "a running image cannot be overwritten, hence the rename"
        );

        fs::write(&layout.source, b"cli 0.1.1").unwrap();
        let outcome = export(&layout, "0.1.1", 1_000).unwrap();

        assert_eq!(outcome.decision, Decision::Export);
        assert_eq!(fs::read(layout.exported_exe()).unwrap(), b"cli 0.1.1");
        assert_eq!(
            fs::read_to_string(layout.dir.join(MARKER)).unwrap(),
            "0.1.1"
        );
        assert_eq!(outcome.still_locked, 1, "the old image is still running");
        assert_eq!(leftovers(&layout), ["acta.exe.old-1000"]);
        assert!(cli.0.try_wait().unwrap().is_none(), "and was not disturbed");

        // Once the old CLI exits, the next start removes its file.
        cli.0.kill().unwrap();
        cli.0.wait().unwrap();
        let mut outcome = export(&layout, "0.1.1", 2_000).unwrap();
        for _ in 0..20 {
            if outcome.still_locked == 0 {
                break;
            }
            // The image is released a moment after the process ends.
            std::thread::sleep(std::time::Duration::from_millis(100));
            outcome = export(&layout, "0.1.1", 2_000).unwrap();
        }
        assert_eq!(outcome.decision, Decision::UpToDate);
        assert_eq!(outcome.still_locked, 0);
        assert!(leftovers(&layout).is_empty());
    }
}
