//! Uninstall hook (Windows; Velopack has no hooks on macOS and Linux):
//! take the CLI off the PATH, stop processes running from the export
//! root, delete it (ADR 0004). A CLI left running would keep its file
//! locked and the directory behind.

/// Whether the process image `image` lives in `dir` (or below it).
/// Windows paths: case-insensitive, either separator.
pub fn image_is_under(dir: &str, image: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").to_lowercase();
    let dir = norm(dir);
    let dir = dir.trim_end_matches('\\');
    let image = norm(image);
    !dir.is_empty()
        && image
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('\\'))
}

/// Undo the export, in this order: PATH entry, running CLIs, files. Returns
/// whether the user environment changed; the caller announces that last
/// (`user_path::announce_environment_change`), so that a slow or hung window
/// can never keep the files from being removed within the hook's time limit.
///
/// `path_value` is `user_path::USER_PATH` outside tests.
#[cfg(windows)]
pub fn run(layout: &crate::cli_export::Layout, path_value: &crate::user_path::PathValue) -> bool {
    use std::time::Duration;

    use crate::log;

    let environment_changed = match crate::user_path::unpublish(layout, path_value) {
        Ok(changed) => {
            log::line(if changed {
                "uninstall: removed from user PATH"
            } else {
                "uninstall: was not on user PATH"
            });
            changed
        }
        Err(err) => {
            log::line(&format!("uninstall: PATH not cleaned: {:?}", err.kind()));
            false
        }
    };

    // Only ever delete the directory that exists for the export alone.
    let root = &layout.removable_root;
    if !layout.dir.starts_with(root) || layout.dir == *root {
        log::line("uninstall: export directory is not inside its root; nothing deleted");
        return environment_changed;
    }

    let stopped = processes::terminate_running_from(&root.to_string_lossy());
    log::line(&format!("uninstall: stopped {stopped} CLI process(es)"));

    // A terminated process releases its image a moment later; the hook has 30 s.
    for attempt in 0..10 {
        match std::fs::remove_dir_all(root) {
            Ok(()) => {
                log::line("uninstall: exported CLI removed");
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
            Err(err) if attempt == 9 => {
                log::line(&format!(
                    "uninstall: exported CLI not removed: {:?}",
                    err.kind()
                ));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
    environment_changed
}

#[cfg(windows)]
mod processes {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        QueryFullProcessImageNameW, TerminateProcess,
    };

    /// Terminate every other process whose image is under `dir`; returns how many.
    pub fn terminate_running_from(dir: &str) -> usize {
        let own_pid = std::process::id();
        let mut stopped = 0;
        // SAFETY: plain Win32 calls. Every handle is checked before use and
        // closed exactly once; every buffer is owned here and its length is
        // passed alongside it.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return 0;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let pid = entry.th32ProcessID;
                if pid != own_pid && pid != 0 {
                    let process = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                        0,
                        pid,
                    );
                    if !process.is_null() {
                        let mut buffer = [0u16; 1024];
                        let mut len = buffer.len() as u32;
                        if QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len)
                            != 0
                        {
                            let image = String::from_utf16_lossy(&buffer[..len as usize]);
                            if super::image_is_under(dir, &image)
                                && TerminateProcess(process, 1) != 0
                            {
                                stopped += 1;
                            }
                        }
                        CloseHandle(process);
                    }
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        stopped
    }
}

#[cfg(test)]
mod tests {
    use super::image_is_under;

    const DIR: &str = r"C:\Users\Alexandre\AppData\Local\ActaCli";

    #[test]
    fn matches_the_exported_cli_and_files_renamed_aside() {
        assert!(image_is_under(DIR, &format!(r"{DIR}\bin\acta.exe")));
        assert!(image_is_under(
            DIR,
            &format!(r"{DIR}\bin\acta.exe.old-17636")
        ));
    }

    #[test]
    fn is_case_insensitive_and_accepts_either_separator() {
        let image = format!(r"{}\BIN\ACTA.EXE", DIR.to_uppercase());
        assert!(image_is_under(DIR, &image));
        assert!(image_is_under(
            &DIR.replace('\\', "/"),
            &format!(r"{DIR}\bin\acta.exe")
        ));
        assert!(image_is_under(
            &format!(r"{DIR}\"),
            &format!(r"{DIR}\bin\acta.exe")
        ));
    }

    #[test]
    fn does_not_match_siblings_or_the_app_itself() {
        assert!(!image_is_under(DIR, &format!(r"{DIR}-other\acta.exe")));
        assert!(!image_is_under(
            DIR,
            r"C:\Users\Alexandre\AppData\Local\Acta\current\Acta.exe"
        ));
        assert!(!image_is_under(
            DIR,
            r"C:\Users\Alexandre\AppData\Local\ActaData\logs\acta.log"
        ));
        assert!(!image_is_under(DIR, DIR));
        assert!(!image_is_under("", r"C:\anything.exe"));
    }

    #[cfg(windows)]
    mod run {
        use std::fs;
        use std::path::Path;

        use winreg::enums::RegType;

        use crate::cli_export::Layout;
        use crate::cli_export::tests::running::run_exported_cli;
        use crate::user_path::tests::registry::Scratch;

        fn layout(root: &Path) -> Layout {
            let removable_root = root.join("ActaCli");
            Layout {
                source: root.join("package").join("acta-cli.exe"),
                dir: removable_root.join("bin"),
                removable_root,
                exe_name: "acta.exe",
                link_dir: None,
            }
        }

        fn encode(text: &str) -> Vec<u8> {
            text.encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect()
        }

        fn text(scratch: &Scratch) -> String {
            let bytes = scratch.get().unwrap().bytes;
            let wide: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .take_while(|&unit| unit != 0)
                .collect();
            String::from_utf16(&wide).unwrap()
        }

        #[test]
        fn removes_the_path_entry_the_running_cli_and_the_export_root() {
            let tmp = tempfile::tempdir().unwrap();
            let layout = layout(tmp.path());
            let data = tmp.path().join("ActaData");
            fs::create_dir_all(&data).unwrap();
            fs::write(data.join("acta.db"), b"database").unwrap();

            let scratch = Scratch::new("uninstall");
            let dir = layout.dir.to_str().unwrap();
            scratch.set(
                encode(&format!(r"%USERPROFILE%\bin;{dir};C:\tools")),
                RegType::REG_EXPAND_SZ,
            );
            let mut cli = run_exported_cli(&layout);
            assert!(cli.0.try_wait().unwrap().is_none(), "the CLI is running");

            let environment_changed = super::super::run(&layout, &scratch.target());

            assert!(environment_changed);
            assert_eq!(text(&scratch), r"%USERPROFILE%\bin;C:\tools");
            assert_eq!(scratch.get().unwrap().vtype, RegType::REG_EXPAND_SZ);
            assert!(cli.0.wait().is_ok(), "the CLI was terminated");
            assert!(!layout.removable_root.exists());
            // Everything beside the root is untouched.
            assert_eq!(fs::read(data.join("acta.db")).unwrap(), b"database");
        }

        #[test]
        fn a_second_run_finds_nothing_to_do() {
            let tmp = tempfile::tempdir().unwrap();
            let layout = layout(tmp.path());
            let scratch = Scratch::new("uninstall-twice");
            scratch.set(encode(r"C:\tools"), RegType::REG_SZ);

            assert!(!super::super::run(&layout, &scratch.target()));

            assert_eq!(text(&scratch), r"C:\tools");
            assert!(tmp.path().exists());
        }

        #[test]
        fn refuses_a_layout_whose_export_is_not_inside_its_root() {
            let tmp = tempfile::tempdir().unwrap();
            let mut layout = layout(tmp.path());
            // A root that is wider than the export: never deleted.
            let precious = tmp.path().join("precious");
            fs::create_dir_all(&precious).unwrap();
            fs::write(precious.join("keep.txt"), b"keep").unwrap();
            layout.removable_root = precious.clone();
            let scratch = Scratch::new("uninstall-refuse");

            super::super::run(&layout, &scratch.target());

            assert!(precious.join("keep.txt").exists());

            // And a root equal to the export directory is refused as well.
            layout.removable_root = layout.dir.clone();
            fs::create_dir_all(&layout.dir).unwrap();
            super::super::run(&layout, &scratch.target());
            assert!(layout.dir.exists());
        }
    }
}
