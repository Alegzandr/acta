//! "Open in VS Code" (PLAN.md, Open a folder in VS Code): where VS Code is
//! installed, and starting it detached on a folder. Offered only when it is
//! found, never an install offer. No shell: the program is started directly
//! with the folder as its one argument (the accepted case of the system
//! file manager), stdin and outputs closed, no console window.
//!
//! Detection takes its directories and `is_file` / `is_dir` closures, so
//! the tests run on fake trees on any host.

use crate::paths::Os;
use crate::prereqs::locate::Bases;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How VS Code is started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VsCode {
    /// A program started with the folder as its argument: `Code.exe` on
    /// Windows, `code` (or `code-insiders`, the Snap's) elsewhere.
    Program(PathBuf),
    /// A macOS app bundle, started with `open -a <app> <folder>`.
    MacApp(PathBuf),
    /// A Flatpak, started with `flatpak run <id> <folder>`.
    Flatpak { flatpak: PathBuf, id: &'static str },
}

const FLATPAK_IDS: [&str; 2] = ["com.visualstudio.code", "com.visualstudio.code-insiders"];

/// VS Code on this machine, if any. `path_dirs`: the directories of the
/// `PATH` (with the registry's on Windows, [`crate::prereqs::locate::system_dirs`]).
pub fn detect_with(
    os: Os,
    path_dirs: &[PathBuf],
    bases: &Bases,
    is_file: impl Fn(&Path) -> bool,
    is_dir: impl Fn(&Path) -> bool,
) -> Option<VsCode> {
    match os {
        Os::Windows => {
            // `code` on the PATH is `<install>\bin\code.cmd`, a batch file:
            // its install's `Code.exe` is started instead, with no shell.
            let from_path = path_dirs.iter().find_map(|d| {
                ["code.cmd", "code-insiders.cmd"]
                    .iter()
                    .find(|n| is_file(&d.join(n)))
                    .and_then(|_| windows_exe_in(d.parent()?, &is_file))
            });
            let mut installs = Vec::new();
            if let Some(lad) = &bases.local_app_data {
                installs.push(lad.join("Programs"));
            }
            installs.extend(bases.program_files.iter().cloned());
            from_path
                .or_else(|| {
                    installs.iter().find_map(|base| {
                        ["Microsoft VS Code", "Microsoft VS Code Insiders"]
                            .iter()
                            .find_map(|n| windows_exe_in(&base.join(n), &is_file))
                    })
                })
                .map(VsCode::Program)
        }
        Os::MacOs => {
            let mut apps = vec![PathBuf::from("/Applications")];
            if let Some(home) = &bases.home {
                apps.push(home.join("Applications"));
            }
            apps.iter()
                .flat_map(|a| {
                    [
                        a.join("Visual Studio Code.app"),
                        a.join("Visual Studio Code - Insiders.app"),
                    ]
                })
                .find(|p| is_dir(p))
                .map(VsCode::MacApp)
                .or_else(|| on_path(path_dirs, &is_file).map(VsCode::Program))
        }
        Os::Linux => on_path(path_dirs, &is_file)
            .or_else(|| {
                ["code", "code-insiders"]
                    .iter()
                    .map(|n| Path::new("/snap/bin").join(n))
                    .find(|p| is_file(p))
            })
            .map(VsCode::Program)
            .or_else(|| {
                let flatpak = path_dirs
                    .iter()
                    .map(|d| d.join("flatpak"))
                    .find(|p| is_file(p))?;
                let mut roots = vec![PathBuf::from("/var/lib/flatpak/app")];
                if let Some(home) = &bases.home {
                    roots.push(home.join(".local/share/flatpak/app"));
                }
                let id = FLATPAK_IDS
                    .into_iter()
                    .find(|id| roots.iter().any(|r| is_dir(&r.join(id))))?;
                Some(VsCode::Flatpak { flatpak, id })
            }),
    }
}

fn windows_exe_in(install: &Path, is_file: &impl Fn(&Path) -> bool) -> Option<PathBuf> {
    ["Code.exe", "Code - Insiders.exe"]
        .iter()
        .map(|n| install.join(n))
        .find(|p| is_file(p))
}

fn on_path(dirs: &[PathBuf], is_file: &impl Fn(&Path) -> bool) -> Option<PathBuf> {
    ["code", "code-insiders"]
        .iter()
        .find_map(|n| dirs.iter().map(|d| d.join(n)).find(|p| is_file(p)))
}

/// [`detect_with`] on this machine. A few file checks: cheap enough for a
/// menu, cached by the caller for the session.
pub fn detect() -> Option<VsCode> {
    detect_with(
        Os::CURRENT,
        &crate::prereqs::locate::system_dirs(),
        &Bases::from_env(),
        crate::prereqs::locate::is_file,
        Path::is_dir,
    )
}

impl VsCode {
    /// The command that opens `folder` (an absolute path, so never taken
    /// for an option).
    pub fn command(&self, folder: &Path) -> io::Result<Command> {
        if !folder.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not an absolute path",
            ));
        }
        let mut c = match self {
            VsCode::Program(p) => Command::new(p),
            VsCode::MacApp(app) => {
                let mut c = Command::new("/usr/bin/open");
                c.arg("-a").arg(app);
                c
            }
            VsCode::Flatpak { flatpak, id } => {
                let mut c = Command::new(flatpak);
                c.arg("run").arg(id);
                c
            }
        };
        c.arg(folder)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Set inside an Electron or VS Code terminal: `Code.exe` would
            // run as plain Node instead of opening a window.
            .env_remove("ELECTRON_RUN_AS_NODE")
            .env_remove("ELECTRON_NO_ATTACH_CONSOLE");
        if let Some(parent) = folder.parent() {
            c.current_dir(parent);
        }
        Ok(c)
    }

    /// Starts VS Code on `folder`, detached: it outlives Acta and is not
    /// waited for. An error is shown as a toast.
    pub fn launch(&self, folder: &Path) -> io::Result<()> {
        let mut c = self.command(folder)?;
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
            let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
            c.creation_flags(base | CREATE_BREAKAWAY_FROM_JOB);
            let child = c.spawn().or_else(|_| {
                c.creation_flags(base);
                c.spawn()
            })?;
            drop(child);
            Ok(())
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            c.process_group(0);
            let mut child = c.spawn()?;
            // Reaped on its own thread: no zombie, nothing waits on it.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn tree(files: &[&str]) -> impl Fn(&Path) -> bool + Clone {
        let set: BTreeSet<PathBuf> = files.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    fn win_bases() -> Bases {
        Bases {
            home: Some(PathBuf::from("C:/Users/Alexandre")),
            local_app_data: Some(PathBuf::from("C:/Users/Alexandre/AppData/Local")),
            program_files: vec![PathBuf::from("C:/Program Files")],
        }
    }

    #[test]
    fn windows_prefers_the_path_install_and_starts_its_exe() {
        let path = [PathBuf::from("D:/Tools/VS Code/bin")];
        let files = tree(&[
            "D:/Tools/VS Code/bin/code.cmd",
            "D:/Tools/VS Code/Code.exe",
            "C:/Program Files/Microsoft VS Code/Code.exe",
        ]);
        let none = tree(&[]);
        assert_eq!(
            detect_with(Os::Windows, &path, &win_bases(), files, none),
            Some(VsCode::Program(PathBuf::from("D:/Tools/VS Code/Code.exe")))
        );
    }

    #[test]
    fn windows_finds_the_user_and_system_installs() {
        let user = tree(&["C:/Users/Alexandre/AppData/Local/Programs/Microsoft VS Code/Code.exe"]);
        assert_eq!(
            detect_with(Os::Windows, &[], &win_bases(), user, tree(&[])),
            Some(VsCode::Program(PathBuf::from(
                "C:/Users/Alexandre/AppData/Local/Programs/Microsoft VS Code/Code.exe"
            )))
        );
        let insiders = tree(&["C:/Program Files/Microsoft VS Code Insiders/Code - Insiders.exe"]);
        assert!(matches!(
            detect_with(Os::Windows, &[], &win_bases(), insiders, tree(&[])),
            Some(VsCode::Program(p)) if p.ends_with("Code - Insiders.exe")
        ));
        // A `code.cmd` with no `Code.exe` next to it is not used.
        let shim = tree(&["C:/npm/code.cmd"]);
        assert_eq!(
            detect_with(
                Os::Windows,
                &[PathBuf::from("C:/npm")],
                &win_bases(),
                shim,
                tree(&[])
            ),
            None
        );
    }

    #[test]
    fn macos_prefers_the_app_bundle() {
        let bases = Bases {
            home: Some(PathBuf::from("/Users/alexandre")),
            ..Bases::default()
        };
        let dirs = tree(&["/Users/alexandre/Applications/Visual Studio Code.app"]);
        assert_eq!(
            detect_with(Os::MacOs, &[], &bases, tree(&[]), dirs),
            Some(VsCode::MacApp(PathBuf::from(
                "/Users/alexandre/Applications/Visual Studio Code.app"
            )))
        );
        let path = [PathBuf::from("/usr/local/bin")];
        assert_eq!(
            detect_with(
                Os::MacOs,
                &path,
                &bases,
                tree(&["/usr/local/bin/code"]),
                tree(&[])
            ),
            Some(VsCode::Program(PathBuf::from("/usr/local/bin/code")))
        );
        assert_eq!(
            detect_with(Os::MacOs, &path, &bases, tree(&[]), tree(&[])),
            None
        );
    }

    #[test]
    fn linux_path_snap_and_flatpak() {
        let bases = Bases {
            home: Some(PathBuf::from("/home/alexandre")),
            ..Bases::default()
        };
        let path = [PathBuf::from("/usr/bin")];
        assert_eq!(
            detect_with(
                Os::Linux,
                &path,
                &bases,
                tree(&["/usr/bin/code-insiders"]),
                tree(&[])
            ),
            Some(VsCode::Program(PathBuf::from("/usr/bin/code-insiders")))
        );
        assert_eq!(
            detect_with(
                Os::Linux,
                &path,
                &bases,
                tree(&["/snap/bin/code"]),
                tree(&[])
            ),
            Some(VsCode::Program(PathBuf::from("/snap/bin/code")))
        );
        assert_eq!(
            detect_with(
                Os::Linux,
                &path,
                &bases,
                tree(&["/usr/bin/flatpak"]),
                tree(&["/home/alexandre/.local/share/flatpak/app/com.visualstudio.code"])
            ),
            Some(VsCode::Flatpak {
                flatpak: PathBuf::from("/usr/bin/flatpak"),
                id: "com.visualstudio.code"
            })
        );
        // Flatpak installed, VS Code not.
        assert_eq!(
            detect_with(
                Os::Linux,
                &path,
                &bases,
                tree(&["/usr/bin/flatpak"]),
                tree(&[])
            ),
            None
        );
    }

    #[test]
    fn the_folder_is_the_one_argument_and_must_be_absolute() {
        let folder = std::env::temp_dir().join("Alexandre project");
        let c = VsCode::Program(PathBuf::from("code"))
            .command(&folder)
            .unwrap();
        let args: Vec<_> = c.get_args().collect();
        assert_eq!(args, vec![folder.as_os_str()]);
        let c = VsCode::MacApp(PathBuf::from("/Applications/Visual Studio Code.app"))
            .command(&folder)
            .unwrap();
        assert_eq!(c.get_program(), "/usr/bin/open");
        assert_eq!(c.get_args().count(), 3);
        let c = VsCode::Flatpak {
            flatpak: PathBuf::from("/usr/bin/flatpak"),
            id: "com.visualstudio.code",
        }
        .command(&folder)
        .unwrap();
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[..2], ["run", "com.visualstudio.code"]);
        assert!(
            VsCode::Program(PathBuf::from("code"))
                .command(Path::new("-relative"))
                .is_err()
        );
    }

    #[test]
    fn launch_starts_the_program_detached() {
        // A stand-in that exits at once: the launch returns without waiting.
        let tmp = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let prog = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("whoami.exe");
        #[cfg(unix)]
        let prog = PathBuf::from("/bin/true");
        VsCode::Program(prog).launch(tmp.path()).unwrap();
        assert!(
            VsCode::Program(tmp.path().join("no-such-code"))
                .launch(tmp.path())
                .is_err()
        );
    }
}
