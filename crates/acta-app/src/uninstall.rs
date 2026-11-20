//! Uninstall hook (Windows; Velopack has no hooks on macOS and Linux):
//! take the CLI off the PATH, stop processes running from the export
//! directory, delete it (ADR 0004). A CLI left running would keep its file
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

#[cfg(windows)]
pub fn run(layout: &crate::cli_export::Layout) {
    use std::time::Duration;

    use crate::log;

    match crate::user_path::unpublish(layout) {
        Ok(done) => log::line(&format!("uninstall: {done}")),
        Err(err) => log::line(&format!("uninstall: PATH not cleaned: {err}")),
    }

    let stopped = processes::terminate_running_from(&layout.dir.to_string_lossy());
    log::line(&format!("uninstall: stopped {stopped} CLI process(es)"));

    // `<LocalAppData>\ActaCli`, the parent of `bin`: remove the whole export root.
    let root = layout.dir.parent().unwrap_or(&layout.dir);
    // A terminated process releases its image a moment later; the hook has 30 s.
    for attempt in 0..10 {
        match std::fs::remove_dir_all(root) {
            Ok(()) => {
                log::line("uninstall: exported CLI removed");
                return;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) if attempt == 9 => {
                log::line(&format!("uninstall: exported CLI not removed: {err}"));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
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

    const DIR: &str = r"C:\Users\Alexandre\AppData\Local\ActaCli\bin";

    #[test]
    fn matches_the_exported_cli_and_files_renamed_aside() {
        assert!(image_is_under(DIR, &format!(r"{DIR}\acta.exe")));
        assert!(image_is_under(DIR, &format!(r"{DIR}\acta.exe.old-17636")));
    }

    #[test]
    fn is_case_insensitive_and_accepts_either_separator() {
        let image = format!(r"{}\ACTA.EXE", DIR.to_uppercase());
        assert!(image_is_under(DIR, &image));
        assert!(image_is_under(
            &DIR.replace('\\', "/"),
            &format!(r"{DIR}\acta.exe")
        ));
        assert!(image_is_under(
            &format!(r"{DIR}\"),
            &format!(r"{DIR}\acta.exe")
        ));
    }

    #[test]
    fn does_not_match_siblings_or_the_app_itself() {
        assert!(!image_is_under(DIR, &format!(r"{DIR}-other\acta.exe")));
        assert!(!image_is_under(
            DIR,
            r"C:\Users\Alexandre\AppData\Local\Acta\current\Acta.exe"
        ));
        assert!(!image_is_under(DIR, DIR));
        assert!(!image_is_under("", r"C:\anything.exe"));
    }
}
