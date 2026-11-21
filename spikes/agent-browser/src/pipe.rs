//! The local pipe between `acta mcp` and the running app (ADR 0010,
//! decision 3): a named pipe whose DACL admits only the current user,
//! refusing network clients, created first-instance so a squatter makes the
//! app fail loudly, and a mutual HMAC handshake on top.

use crate::auth;
use serde_json::{Value, json};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
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
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, PWSTR};

/// The current user's SID as a string (`S-1-5-21-...`).
pub fn user_sid() -> windows::core::Result<String> {
    // SAFETY: plain token queries into a buffer sized by the first call.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
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

/// One pipe per user: the name carries the SID, so two users on one machine
/// never meet. The spike's name differs from the product's.
pub fn pipe_name(sid: &str) -> String {
    format!(r"\\.\pipe\acta-spike-{sid}-agent-browser")
}

/// Owner-only DACL: the user gets full access, network logons are denied
/// explicitly, nobody else is listed (so Everyone and anonymous, which the
/// default DACL lets read, get nothing). `P` blocks inherited ACEs.
pub fn sddl(sid: &str) -> String {
    format!("D:P(D;;GA;;;NU)(A;;GA;;;{sid})")
}

pub struct Server {
    name: HSTRING,
    sd: PSECURITY_DESCRIPTOR,
    first: bool,
}

// The descriptor is read-only after creation.
unsafe impl Send for Server {}

impl Server {
    pub fn new(sid: &str) -> windows::core::Result<Server> {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: a valid SDDL string; the descriptor lives as long as the
        // server (leaked with the process in the spike).
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &HSTRING::from(sddl(sid)),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )?;
        }
        Ok(Server {
            name: HSTRING::from(pipe_name(sid)),
            sd,
            first: true,
        })
    }

    /// Creates the next instance and waits for a client. The first instance
    /// is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`: if anyone else
    /// already owns the name this fails instead of joining their pipe.
    pub fn accept(&mut self) -> windows::core::Result<(File, u32, String)> {
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
        // by the returned File.
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
                return Err(windows::core::Error::from_win32());
            }
            self.first = false;
            let dacl = read_sddl(h);
            if let Err(e) = ConnectNamedPipe(h, None)
                && e.code() != ERROR_PIPE_CONNECTED.to_hresult()
            {
                return Err(e);
            }
            let mut pid = 0u32;
            let _ = GetNamedPipeClientProcessId(h, &mut pid);
            Ok((File::from_raw_handle(h.0), pid, dacl))
        }
    }
}

/// The DACL actually on the pipe, as SDDL, read back from the handle.
fn read_sddl(h: HANDLE) -> String {
    // SAFETY: queries on a live handle; strings freed with LocalFree.
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

pub struct Conn {
    reader: BufReader<File>,
    writer: File,
}

impl Conn {
    pub fn new(f: File) -> std::io::Result<Conn> {
        Ok(Conn {
            writer: f.try_clone()?,
            reader: BufReader::new(f),
        })
    }

    pub fn send(&mut self, v: &Value) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(v)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()
    }

    /// One request line, capped at 1 MB so a client cannot balloon memory.
    pub fn recv(&mut self) -> std::io::Result<Option<Value>> {
        let mut line = String::new();
        let n = (&mut self.reader).take(1 << 20).read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        serde_json::from_str(&line)
            .map(Some)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

use std::io::Read as _;

/// Server side of the handshake: true when the client proved the key.
pub fn server_handshake(c: &mut Conn, key: &[u8; 32]) -> std::io::Result<bool> {
    let s = auth::nonce();
    c.send(&json!({ "protocol": auth::PROTOCOL, "nonce": auth::hex(&s) }))?;
    let Some(hello) = c.recv()? else {
        return Ok(false);
    };
    let cn = hello["nonce"]
        .as_str()
        .and_then(auth::unhex)
        .unwrap_or_default();
    let proof = hello["mac"].as_str().unwrap_or("");
    if cn.len() != 32 || !auth::verify(key, "client", &s, &cn, proof) {
        c.send(&json!({ "error": "not authenticated" }))?;
        return Ok(false);
    }
    c.send(&json!({ "mac": auth::server_mac(key, &s, &cn) }))?;
    Ok(true)
}

/// Client side: Ok when the server proved the key too.
pub fn client_handshake(c: &mut Conn, key: &[u8; 32]) -> Result<(), String> {
    let hello = c.recv().map_err(|e| e.to_string())?.ok_or("closed")?;
    if hello["protocol"] != auth::PROTOCOL {
        return Err("wrong protocol".into());
    }
    let s = hello["nonce"]
        .as_str()
        .and_then(auth::unhex)
        .ok_or("no nonce")?;
    let cn = auth::nonce();
    c.send(&json!({ "nonce": auth::hex(&cn), "mac": auth::client_mac(key, &s, &cn) }))
        .map_err(|e| e.to_string())?;
    let r = c
        .recv()
        .map_err(|e| e.to_string())?
        .ok_or("closed during handshake")?;
    if let Some(e) = r["error"].as_str() {
        return Err(format!("server refused: {e}"));
    }
    if !auth::verify(key, "server", &s, &cn, r["mac"].as_str().unwrap_or("")) {
        return Err("server did not prove the key: not the Acta app".into());
    }
    Ok(())
}

/// Opens the pipe as a client. `SECURITY_IDENTIFICATION` keeps the server
/// from impersonating this process beyond identifying it.
pub fn connect(name: &str) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;
    let mut last = None;
    for _ in 0..150 {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(name)
        {
            Ok(f) => return Ok(f),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no pipe")))
}
