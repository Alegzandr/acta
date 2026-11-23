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
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, GetAce, GetLengthSid,
    GetSecurityDescriptorControl, GetTokenInformation, IsValidSid, PSECURITY_DESCRIPTOR, PSID,
    SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT,
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

/// The tag of a data directory for the mutex: the first 8 hex digits of
/// the SHA-256 of its path. It says nothing about the path. The mutex lives
/// in the session's `Local\` namespace, out of other users' reach, and is
/// needed before the database (and its key) is open.
fn tag(data_dir: &Path) -> String {
    let path = data_dir.to_string_lossy().to_lowercase();
    let digest = Sha256::digest(path.as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// `\\.\pipe\acta-<user SID>-browser-<tag>`, the tag being the first 16 hex
/// digits of HMAC-SHA256(pipe key, "pipe name v1" and the data directory).
/// The SID keeps users apart; the data directory keeps a development or test
/// instance from meeting the installed app; the key, which only the app
/// and `acta mcp browser` hold, keeps another local user from computing the
/// name in advance. Pipe names are listable, so this holds only until the
/// name is observed; squatting it then is a denial of service, never
/// impersonation (the first-instance flag and the key handshake).
pub fn pipe_name(sid: &str, key: &[u8; 32], data_dir: &Path) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(b"pipe name v1");
    m.update(data_dir.to_string_lossy().to_lowercase().as_bytes());
    let tag = crate::auth::hex(&m.finalize().into_bytes()[..8]);
    format!(r"\\.\pipe\acta-{sid}-browser-{tag}")
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

/// Full access to a file object: what `GA` maps to on a pipe.
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
/// NT AUTHORITY\NETWORK.
const NETWORK_SID: &str = "S-1-5-2";

/// The rule of [`sddl`], checked on a pipe handle ACE by ACE rather than as
/// text: SDDL text names some users by alias (`LA` for the built-in
/// administrator, as on CI runners), so the same DACL can read two ways.
/// Protected, exactly two ACEs: network denied everything, then `sid`
/// allowed everything.
pub fn dacl_is_ours(h: HANDLE, sid: &str) -> bool {
    // SAFETY: queries on a live handle; the descriptor (which owns the
    // DACL and its ACEs) is freed with LocalFree after the last read.
    unsafe {
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        if GetSecurityInfo(
            h,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            Some(&mut sd),
        )
        .is_err()
            || dacl.is_null()
        {
            return false;
        }
        let ok = (|| {
            let (mut control, mut revision) = (0u16, 0u32);
            GetSecurityDescriptorControl(sd, &mut control, &mut revision).ok()?;
            if control & SE_DACL_PROTECTED.0 == 0 || (*dacl).AceCount != 2 {
                return None;
            }
            let ace = |i: u32| -> Option<Ace> {
                let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
                GetAce(dacl, i, &mut p).ok()?;
                read_ace(p)
            };
            rule_holds(&ace(0)?, &ace(1)?, sid).then_some(())
        })()
        .is_some();
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        ok
    }
}

/// One ACE of a pipe's DACL, as the rule needs it.
#[derive(Debug, PartialEq, Eq)]
struct Ace {
    kind: u8,
    flags: u8,
    mask: u32,
    sid: String,
}

const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

/// Reads an allowed or denied ACE at `p`; `None` for any other type, or a
/// SID that is invalid or does not fit inside the ACE.
///
/// # Safety
/// `p` points to an ACE of at least `ACE_HEADER` size whose `AceSize` bytes
/// are readable (as `GetAce` returns).
unsafe fn read_ace(p: *const core::ffi::c_void) -> Option<Ace> {
    // SAFETY: the caller's contract; the header is read before anything
    // else, and the SID only once its type, validity and length are known.
    unsafe {
        let header = &*(p as *const ACE_HEADER);
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE && header.AceType != ACCESS_DENIED_ACE_TYPE {
            return None;
        }
        // Allowed and denied ACEs share this layout.
        let fixed = std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart);
        let size = usize::from(header.AceSize);
        // The SID's fixed part (revision, count, authority) must fit before
        // IsValidSid reads its sub-authority count.
        if size < fixed + 8 {
            return None;
        }
        let ace = &*(p as *const ACCESS_ALLOWED_ACE);
        let sid_ptr = PSID(&ace.SidStart as *const u32 as *mut core::ffi::c_void);
        let count = usize::from(*(sid_ptr.0 as *const u8).add(1));
        if fixed + 8 + 4 * count > size || !IsValidSid(sid_ptr).as_bool() {
            return None;
        }
        if fixed + GetLengthSid(sid_ptr) as usize > size {
            return None;
        }
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(sid_ptr, &mut s).ok()?;
        let text = s.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
        Some(Ace {
            kind: header.AceType,
            flags: header.AceFlags,
            mask: ace.Mask,
            sid: text,
        })
    }
}

/// Network denied everything, then `sid` allowed everything, both without
/// flags (an inherit-only ACE would not apply to the pipe).
fn rule_holds(first: &Ace, second: &Ace, sid: &str) -> bool {
    *first
        == Ace {
            kind: ACCESS_DENIED_ACE_TYPE,
            flags: 0,
            mask: FILE_ALL_ACCESS,
            sid: NETWORK_SID.into(),
        }
        && *second
            == Ace {
                kind: ACCESS_ALLOWED_ACE_TYPE,
                flags: 0,
                mask: FILE_ALL_ACCESS,
                sid: sid.into(),
            }
}

pub struct Server {
    name: HSTRING,
    sid: String,
    sd: PSECURITY_DESCRIPTOR,
    created_any: bool,
    /// The instance waiting for the next client.
    next: Option<File>,
    /// Tests: make the instance after the first fail.
    #[cfg(test)]
    fail_next: bool,
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
            #[cfg(test)]
            fail_next: false,
        })
    }

    /// Creates one instance and checks its DACL. The first one is created
    /// with `FILE_FLAG_FIRST_PIPE_INSTANCE`: if the name is already taken
    /// this fails instead of joining someone else's pipe.
    fn create(&mut self) -> std::io::Result<(File, String)> {
        #[cfg(test)]
        if self.fail_next && self.created_any {
            return Err(std::io::Error::other("injected failure"));
        }
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
        let h = HANDLE(file.as_raw_handle());
        let dacl = read_sddl(h);
        if !dacl_is_ours(h, &self.sid) {
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
        loop {
            // SAFETY: a live pipe instance owned by `file`.
            match unsafe { ConnectNamedPipe(h, None) } {
                Ok(()) => break,
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => break,
                // A client connected and closed before this call: the
                // instance is reset and waits for the next one, so the name
                // stays served.
                Err(e) if e.code() == ERROR_NO_DATA.to_hresult() => {
                    // SAFETY: as above.
                    unsafe { DisconnectNamedPipe(h)? };
                }
                Err(e) => return Err(e.into()),
            }
        }
        let dacl = read_sddl(h);
        // The next instance exists before this one is handed off. If it
        // cannot be made, nothing is served: the connected client is
        // dropped and the caller stops serving (and says why).
        self.next = Some(self.create()?.0);
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
        let key = [7u8; 32];
        let a = pipe_name(
            "S-1-5-21-1",
            &key,
            Path::new(r"C:\Users\Alexandre\AppData\Local\ActaData"),
        );
        let b = pipe_name(
            "S-1-5-21-1",
            &key,
            Path::new(r"c:\users\alexandre\appdata\local\actadata"),
        );
        let c = pipe_name("S-1-5-21-1", &key, Path::new(r"C:\Temp\acta-test"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(r"\\.\pipe\acta-S-1-5-21-1-browser-"));
        assert!(!a.to_lowercase().contains("alexandre"));
        assert_eq!(sddl("S-1-5-21-1"), "D:P(D;;GA;;;NU)(A;;GA;;;S-1-5-21-1)");
        let m = mutex_name("S-1-5-21-1", Path::new(r"C:\Temp\acta-test"));
        assert!(m.starts_with(r"Local\acta-S-1-5-21-1-app-"));
    }

    /// Security audit: the old tag was the SHA-256 of the data directory,
    /// which another local user can compute and use to create the pipe
    /// first. The tag now needs the key both sides hold.
    #[test]
    fn the_pipe_name_needs_the_key() {
        let dir = Path::new(r"C:\Users\Alexandre\AppData\Local\Acta");
        let sid = "S-1-5-21-1";
        let ours = pipe_name(sid, &[1u8; 32], dir);
        // The app and `acta mcp browser` derive it the same way.
        assert_eq!(ours, pipe_name(sid, &[1u8; 32], dir));
        assert_ne!(ours, pipe_name(sid, &[2u8; 32], dir));
        let old = format!(r"\\.\pipe\acta-{sid}-browser-{}", tag(dir));
        assert_ne!(ours, old);
        assert!(!ours.contains(&tag(dir)));
        let tag = ours.rsplit('-').next().unwrap();
        assert_eq!(tag.len(), 16);
        assert!(tag.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    /// Known answers, computed independently: the pipe and mutex names must
    /// not change between builds, or the app and `acta mcp browser` of
    /// different versions stop finding each other.
    #[test]
    fn names_are_stable() {
        let dir = Path::new(r"C:\Users\Alexandre\AppData\Local\Acta");
        assert_eq!(
            pipe_name("S-1-5-21-1", &[7u8; 32], dir),
            r"\\.\pipe\acta-S-1-5-21-1-browser-0dee4a42f0115968"
        );
        assert_eq!(
            mutex_name("S-1-5-21-1", dir),
            r"Local\acta-S-1-5-21-1-app-93476423"
        );
    }

    /// Security audit: a client that connects and closes before the server
    /// waits on the instance made `ConnectNamedPipe` fail with
    /// `ERROR_NO_DATA`, and the server stopped for good.
    #[test]
    fn a_client_gone_before_the_wait_does_not_stop_the_server() {
        let name = unique_name("nodata");
        let mut server = Server::new(&name, &user_sid().unwrap()).unwrap();
        server.next = Some(server.create().unwrap().0);
        let early = connect(&name, Duration::from_secs(2)).unwrap();
        drop(early);
        let t = std::thread::spawn(move || {
            let accepted = server.accept().map(|_| ());
            (accepted, server)
        });
        std::thread::sleep(Duration::from_millis(100));
        let client = connect(&name, Duration::from_secs(2));
        let (accepted, _server) = t.join().unwrap();
        assert!(accepted.is_ok(), "{accepted:?}");
        assert!(client.is_ok());
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

    /// A pipe name no other test, run or process uses.
    fn unique_name(test: &str) -> String {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b).unwrap();
        let tag: String = b.iter().map(|x| format!("{x:02x}")).collect();
        format!(r"\\.\pipe\acta-test-{test}-{tag}")
    }

    /// An ACE in memory: header, mask, then `sid` bytes; `size` overrides
    /// `AceSize`.
    fn raw_ace(kind: u8, flags: u8, sid: &[u8], size: Option<u16>) -> Vec<u32> {
        let mut bytes = vec![kind, flags, 0, 0];
        let len = (8 + sid.len()) as u16;
        bytes[2..4].copy_from_slice(&size.unwrap_or(len).to_le_bytes());
        bytes.extend_from_slice(&FILE_ALL_ACCESS.to_le_bytes());
        bytes.extend_from_slice(sid);
        bytes.resize(bytes.len().div_ceil(4) * 4 + 64, 0);
        // u32 storage keeps the ACE aligned as Windows lays it out.
        bytes
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// S-1-5-2 (NETWORK).
    const NETWORK: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 5, 2, 0, 0, 0];

    fn read(buf: &[u32]) -> Option<Ace> {
        // SAFETY: `buf` holds the ACE and slack beyond its AceSize.
        unsafe { read_ace(buf.as_ptr().cast()) }
    }

    #[test]
    fn aces_are_read_only_when_their_type_and_sid_are_sound() {
        let ok = read(&raw_ace(ACCESS_DENIED_ACE_TYPE, 0, &NETWORK, None)).unwrap();
        assert_eq!((ok.kind, ok.flags, ok.sid.as_str()), (1, 0, NETWORK_SID));
        // Another ACE type (an object ACE, 5) is not read as allowed or denied.
        assert_eq!(read(&raw_ace(5, 0, &NETWORK, None)), None);
        // A SID claiming more sub-authorities than the ACE holds.
        let mut long = NETWORK;
        long[1] = 15;
        assert_eq!(
            read(&raw_ace(ACCESS_ALLOWED_ACE_TYPE, 0, &long, None)),
            None
        );
        // An invalid SID (revision 2).
        let mut bad = NETWORK;
        bad[0] = 2;
        assert_eq!(read(&raw_ace(ACCESS_ALLOWED_ACE_TYPE, 0, &bad, None)), None);
        // An AceSize too small for its SID.
        assert_eq!(
            read(&raw_ace(ACCESS_DENIED_ACE_TYPE, 0, &NETWORK, Some(16))),
            None
        );
    }

    #[test]
    fn the_rule_needs_both_aces_without_flags() {
        let sid = "S-1-5-21-1-2-3-1001";
        let ace = |kind, flags, sid: &str| Ace {
            kind,
            flags,
            mask: FILE_ALL_ACCESS,
            sid: sid.into(),
        };
        let deny = ace(ACCESS_DENIED_ACE_TYPE, 0, NETWORK_SID);
        let allow = ace(ACCESS_ALLOWED_ACE_TYPE, 0, sid);
        assert!(rule_holds(&deny, &allow, sid));
        // INHERIT_ONLY (8): a deny that does not apply to the pipe.
        assert!(!rule_holds(
            &ace(ACCESS_DENIED_ACE_TYPE, 8, NETWORK_SID),
            &allow,
            sid
        ));
        assert!(!rule_holds(
            &deny,
            &ace(ACCESS_ALLOWED_ACE_TYPE, 2, sid),
            sid
        ));
        assert!(!rule_holds(&deny, &allow, "S-1-5-21-1-2-3-500"));
        assert!(!rule_holds(&allow, &deny, sid));
    }

    #[test]
    fn our_dacl_is_recognised_on_what_we_create() {
        let sid = user_sid().unwrap();
        let mut server = Server::new(&unique_name("dacl"), &sid).unwrap();
        let (file, text) = server.create().unwrap();
        assert!(dacl_is_ours(HANDLE(file.as_raw_handle()), &sid), "{text}");
        // Another user's SID is not ours.
        assert!(!dacl_is_ours(
            HANDLE(file.as_raw_handle()),
            "S-1-5-21-1-2-3-500"
        ));
    }

    #[test]
    fn no_client_is_served_without_an_instance_behind_it() {
        let name = unique_name("next");
        let mut server = Server::new(&name, &user_sid().unwrap()).unwrap();
        server.fail_next = true;
        let t = std::thread::spawn(move || server.accept().map(|_| ()));
        let client = connect(&name, Duration::from_secs(2));
        let accepted = t.join().unwrap();
        assert_eq!(accepted.unwrap_err().to_string(), "injected failure");
        drop(client);
    }

    #[test]
    fn our_own_process_runs_as_us() {
        assert_eq!(
            process_user(Some(std::process::id())).unwrap(),
            user_sid().unwrap()
        );
    }
}
