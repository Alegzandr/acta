//! The named pipe between `acta mcp browser` and the app (ADR 0010,
//! decision 3), Windows only: a DACL that admits the current user alone and
//! denies network logons, checked on every instance; remote clients
//! rejected; the first instance created with `FILE_FLAG_FIRST_PIPE_INSTANCE`
//! so a squatter makes the app fail loudly; the next instance created before
//! a connected one is handed off, so the name never disappears while the app
//! runs; clients that allow identification only and check that the server
//! process runs as their own user. Also the app's named mutex, which tells
//! `acta mcp browser` the app is running (or starting) without the pipe.

use std::fs::File;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, OpenMutexW, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZATION_SYNCHRONIZE,
};
use windows::core::{HSTRING, PWSTR};

/// The SID of a token's user, as a string.
fn token_user(token: HANDLE) -> std::io::Result<String> {
    // SAFETY: token queries into a buffer sized by the first call; the
    // string is freed with LocalFree.
    unsafe {
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            len,
            &mut len,
        )?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s)?;
        let out = s.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
        Ok(out)
    }
}

/// The user SID of process `pid` (or of this process).
fn process_user(pid: Option<u32>) -> std::io::Result<String> {
    // SAFETY: handles opened here are closed here.
    unsafe {
        let process = match pid {
            Some(pid) => OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)?,
            None => GetCurrentProcess(),
        };
        let mut token = HANDLE::default();
        let opened = OpenProcessToken(process, TOKEN_QUERY, &mut token);
        if pid.is_some() {
            let _ = CloseHandle(process);
        }
        opened?;
        let user = token_user(token);
        let _ = CloseHandle(token);
        user
    }
}

/// The current user's SID as a string (`S-1-5-21-...`).
pub fn user_sid() -> std::io::Result<String> {
    process_user(None)
}

/// The tag of a data directory: the first 8 hex digits of the SHA-256 of
/// its path. It says nothing about the path.
fn tag(data_dir: &Path) -> String {
    let path = data_dir.to_string_lossy().to_lowercase();
    let digest = Sha256::digest(path.as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// `\\.\pipe\acta-<user SID>-browser-<data dir tag>`. The SID keeps users
/// apart; the tag keeps a development or test instance from meeting the
/// installed app.
pub fn pipe_name(sid: &str, data_dir: &Path) -> String {
    format!(r"\\.\pipe\acta-{sid}-browser-{}", tag(data_dir))
}

/// `Local\acta-<user SID>-app-<data dir tag>`: held by the app for its whole
/// life, so `acta mcp browser` never starts a second one.
pub fn mutex_name(sid: &str, data_dir: &Path) -> String {
    format!(r"Local\acta-{sid}-app-{}", tag(data_dir))
}

/// Network logons denied, the user admitted, nothing else listed and
/// nothing inherited (`P`): Everyone and anonymous, which the default DACL
/// lets read, get nothing.
pub fn sddl(sid: &str) -> String {
    format!("D:P(D;;GA;;;NU)(A;;GA;;;{sid})")
}

/// The DACL as Windows reads it back (generic rights mapped to file rights).
pub fn expected_dacl(sid: &str) -> String {
    format!("D:P(D;;FA;;;NU)(A;;FA;;;{sid})")
}

pub struct Server {
    name: HSTRING,
    sid: String,
    sd: PSECURITY_DESCRIPTOR,
    created_any: bool,
    /// The instance waiting for the next client.
    next: Option<File>,
}

// SAFETY: the descriptor is read-only after creation and freed in Drop.
unsafe impl Send for Server {}

impl Drop for Server {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptor... with LocalAlloc.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.sd.0)));
        }
    }
}

/// One connected instance and the DACL read back from its handle.
pub struct Accepted {
    pub file: File,
    pub dacl: String,
}

impl Server {
    pub fn new(name: &str, sid: &str) -> std::io::Result<Server> {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: a valid SDDL string; the descriptor is freed in Drop.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &HSTRING::from(sddl(sid)),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )?;
        }
        Ok(Server {
            name: HSTRING::from(name),
            sid: sid.to_string(),
            sd,
            created_any: false,
            next: None,
        })
    }

    /// Creates one instance and checks its DACL. The first one is created
    /// with `FILE_FLAG_FIRST_PIPE_INSTANCE`: if the name is already taken
    /// this fails instead of joining someone else's pipe.
    fn create(&mut self) -> std::io::Result<(File, String)> {
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.sd.0,
            bInheritHandle: false.into(),
        };
        let mut mode = PIPE_ACCESS_DUPLEX;
        if !self.created_any {
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        // SAFETY: a valid name and security attributes; the handle is owned
        // by the returned File.
        let file = unsafe {
            let h = CreateNamedPipeW(
                &self.name,
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                Some(&sa),
            );
            if h.is_invalid() {
                return Err(std::io::Error::last_os_error());
            }
            File::from_raw_handle(h.0)
        };
        self.created_any = true;
        let dacl = read_sddl(HANDLE(file.as_raw_handle()));
        if dacl != expected_dacl(&self.sid) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the pipe instance does not carry the expected DACL",
            ));
        }
        Ok((file, dacl))
    }

    /// Waits for a client on the waiting instance, then creates the next
    /// one before handing this one off.
    pub fn accept(&mut self) -> std::io::Result<Accepted> {
        let file = match self.next.take() {
            Some(f) => f,
            None => self.create()?.0,
        };
        let h = HANDLE(file.as_raw_handle());
        // SAFETY: a live pipe instance owned by `file`.
        if let Err(e) = unsafe { ConnectNamedPipe(h, None) }
            && e.code() != ERROR_PIPE_CONNECTED.to_hresult()
        {
            return Err(e.into());
        }
        let dacl = read_sddl(h);
        // The next instance exists before this one is handed off; if it
        // cannot be made now, the next accept tries again.
        self.next = self.create().map(|(f, _)| f).ok();
        Ok(Accepted { file, dacl })
    }
}

/// The DACL on a handle, as SDDL.
fn read_sddl(h: HANDLE) -> String {
    // SAFETY: queries on a live handle; buffers freed with LocalFree.
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        if GetSecurityInfo(
            h,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            Some(&mut sd),
        )
        .is_err()
        {
            return "?".into();
        }
        let mut s = PWSTR::null();
        let out = if ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut s,
            None,
        )
        .is_ok()
        {
            s.to_string().unwrap_or_default()
        } else {
            "?".into()
        };
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        out
    }
}

/// Opens the pipe as a client, retrying while every instance is busy, up to
/// `wait`, then checks that the server process runs as this user.
/// `SECURITY_IDENTIFICATION` keeps the server from impersonating the client
/// beyond identifying it. `NotFound` when nobody served the name for half a
/// second.
pub fn connect(name: &str, wait: Duration) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;
    const ERROR_PIPE_BUSY: i32 = 231;
    const NOT_FOUND_GRACE: Duration = Duration::from_millis(500);
    let started = Instant::now();
    let end = started + wait;
    let file = loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
            .open(name)
        {
            Ok(f) => break f,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && started.elapsed() < NOT_FOUND_GRACE.min(wait) =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
    };
    let mut pid = 0u32;
    // SAFETY: a query on the pipe handle owned by `file`.
    unsafe { GetNamedPipeServerProcessId(HANDLE(file.as_raw_handle()), &mut pid)? };
    if process_user(Some(pid))? != user_sid()? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the pipe's server runs as another user",
        ));
    }
    Ok(file)
}

/// The app's mutex, held while it lives.
pub struct AppMutex(HANDLE);

// SAFETY: a kernel handle, only closed in Drop.
unsafe impl Send for AppMutex {}

impl AppMutex {
    pub fn hold(name: &str) -> std::io::Result<AppMutex> {
        // SAFETY: a named mutex in this session's namespace; not owned, only
        // its existence matters.
        let h = unsafe { CreateMutexW(None, false, &HSTRING::from(name))? };
        Ok(AppMutex(h))
    }

    /// Creates the mutex only if nobody holds it: `Some` means this process
    /// is the one to start the app, and keeps the mutex until the app holds
    /// it too. Atomic, so two clients never both start one.
    pub fn claim(name: &str) -> std::io::Result<Option<AppMutex>> {
        let held = AppMutex::hold(name)?;
        // SAFETY: reads the error CreateMutexW just set.
        let existed = unsafe { windows::Win32::Foundation::GetLastError() }
            == windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        Ok((!existed).then_some(held))
    }
}

impl Drop for AppMutex {
    fn drop(&mut self) {
        // SAFETY: the handle CreateMutexW returned.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Whether some process holds the app's mutex: the app runs or is starting.
pub fn app_running(name: &str) -> bool {
    // SAFETY: a lookup by name; the handle is closed at once.
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &HSTRING::from(name)) } {
        Ok(h) => {
            // SAFETY: as above.
            unsafe {
                let _ = CloseHandle(h);
            }
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_the_user_and_a_tag_of_the_data_dir() {
        let a = pipe_name(
            "S-1-5-21-1",
            Path::new(r"C:\Users\Alexandre\AppData\Local\ActaData"),
        );
        let b = pipe_name(
            "S-1-5-21-1",
            Path::new(r"c:\users\alexandre\appdata\local\actadata"),
        );
        let c = pipe_name("S-1-5-21-1", Path::new(r"C:\Temp\acta-test"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(r"\\.\pipe\acta-S-1-5-21-1-browser-"));
        assert!(!a.to_lowercase().contains("alexandre"));
        assert_eq!(sddl("S-1-5-21-1"), "D:P(D;;GA;;;NU)(A;;GA;;;S-1-5-21-1)");
        let m = mutex_name("S-1-5-21-1", Path::new(r"C:\Temp\acta-test"));
        assert!(m.starts_with(r"Local\acta-S-1-5-21-1-app-"));
        assert!(m.ends_with(&c[c.len() - 8..]));
    }

    #[test]
    fn the_app_mutex_is_seen_while_held() {
        let name = format!(r"Local\acta-test-{}", std::process::id());
        assert!(!app_running(&name));
        let held = AppMutex::hold(&name).unwrap();
        assert!(app_running(&name));
        assert!(
            AppMutex::claim(&name).unwrap().is_none(),
            "held: not claimed"
        );
        drop(held);
        let claimed = AppMutex::claim(&name).unwrap();
        assert!(claimed.is_some() && app_running(&name));
        drop(claimed);
        assert!(!app_running(&name));
    }

    #[test]
    fn our_own_process_runs_as_us() {
        assert_eq!(
            process_user(Some(std::process::id())).unwrap(),
            user_sid().unwrap()
        );
    }
}
