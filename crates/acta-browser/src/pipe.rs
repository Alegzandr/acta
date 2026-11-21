//! The named pipe between `acta mcp browser` and the app (ADR 0010,
//! decision 3), Windows only: a DACL that admits the current user alone and
//! denies network logons, remote clients rejected, the first instance
//! created with `FILE_FLAG_FIRST_PIPE_INSTANCE` so a squatter makes the app
//! fail loudly, and clients that allow identification only.

use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, LocalFree};
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
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, PWSTR};

/// The current user's SID as a string (`S-1-5-21-...`).
pub fn user_sid() -> std::io::Result<String> {
    // SAFETY: token queries into a buffer sized by the first call; the
    // string is freed with LocalFree.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        let got = GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            len,
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(token);
        got?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s)?;
        let out = s.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
        Ok(out)
    }
}

/// `\\.\pipe\acta-<user SID>-browser-<data dir tag>`. The SID keeps users
/// apart; the tag, the first 8 hex digits of the SHA-256 of the data
/// directory's path, keeps a development or test instance from meeting the
/// installed app. It says nothing about the path.
pub fn pipe_name(sid: &str, data_dir: &Path) -> String {
    let path = data_dir.to_string_lossy().to_lowercase();
    let digest = Sha256::digest(path.as_bytes());
    let tag: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
    format!(r"\\.\pipe\acta-{sid}-browser-{tag}")
}

/// Network logons denied, the user admitted, nothing else listed and
/// nothing inherited (`P`): Everyone and anonymous, which the default DACL
/// lets read, get nothing.
pub fn sddl(sid: &str) -> String {
    format!("D:P(D;;GA;;;NU)(A;;GA;;;{sid})")
}

pub struct Server {
    name: HSTRING,
    sd: PSECURITY_DESCRIPTOR,
    first: bool,
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
            sd,
            first: true,
        })
    }

    /// Creates the next instance and waits for a client. The first one is
    /// created with `FILE_FLAG_FIRST_PIPE_INSTANCE`: if the name is already
    /// taken this fails instead of joining someone else's pipe.
    pub fn accept(&mut self) -> std::io::Result<Accepted> {
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.sd.0,
            bInheritHandle: false.into(),
        };
        let mut mode = PIPE_ACCESS_DUPLEX;
        if self.first {
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        // SAFETY: a valid name and security attributes; the handle is owned
        // by the returned File (or closed on error).
        unsafe {
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
            self.first = false;
            let file = File::from_raw_handle(h.0);
            let dacl = read_sddl(h);
            if let Err(e) = ConnectNamedPipe(h, None)
                && e.code() != ERROR_PIPE_CONNECTED.to_hresult()
            {
                return Err(e.into());
            }
            Ok(Accepted { file, dacl })
        }
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
/// `wait`. `SECURITY_IDENTIFICATION` keeps the server from impersonating the
/// client beyond identifying it. `NotFound` when nobody served the name for half a second.
pub fn connect(name: &str, wait: Duration) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;
    const ERROR_PIPE_BUSY: i32 = 231;
    const NOT_FOUND_GRACE: Duration = Duration::from_millis(500);
    let started = Instant::now();
    let end = started + wait;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
            .open(name)
        {
            Ok(f) => return Ok(f),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(50));
            }
            // Between two instances the name is briefly absent: only a name
            // absent for half a second means nobody serves it.
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && started.elapsed() < NOT_FOUND_GRACE.min(wait) =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
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
    }
}
