//! JSON lines between `acta mcp browser` and the app, and the handshake on
//! them. Generic over any byte stream: the named pipe in the product, an
//! in-memory pair in tests.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};

use crate::auth;

/// A request line is small (text is capped at 2,000 chars).
pub const REQUEST_MAX: u64 = 64 * 1024;
/// A reply can carry a JPEG.
pub const REPLY_MAX: u64 = 16 * 1024 * 1024;

pub struct Conn<R: Read, W: Write> {
    reader: BufReader<R>,
    writer: W,
}

impl<R: Read, W: Write> Conn<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Conn {
            reader: BufReader::new(reader),
            writer,
        }
    }

    pub fn send<T: Serialize>(&mut self, v: &T) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(v)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()
    }

    /// One line, at most `max` bytes; `None` at the end of the stream.
    pub fn recv<T: DeserializeOwned>(&mut self, max: u64) -> std::io::Result<Option<T>> {
        let mut line = Vec::new();
        let n = (&mut self.reader).take(max).read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if line.last() != Some(&b'\n') && n as u64 >= max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "line too long",
            ));
        }
        serde_json::from_slice(&line)
            .map(Some)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// What a client says about itself once authenticated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hello {
    /// The session driving the browser, as the header shows it: the focus
    /// topic or epic id (`T-42`), when there is one.
    pub session: Option<String>,
}

/// Server side: `Ok(hello)` once the client proved the key and the server
/// proved it back; `Err` with what to log (never key material).
pub fn server_handshake<R: Read, W: Write>(
    c: &mut Conn<R, W>,
    key: &[u8; 32],
) -> Result<Hello, String> {
    let s = auth::nonce();
    c.send(&json!({ "protocol": auth::PROTOCOL, "nonce": auth::hex(&s) }))
        .map_err(|e| e.to_string())?;
    let hello: Value = c
        .recv(4096)
        .map_err(|e| e.to_string())?
        .ok_or("closed before the handshake")?;
    let cn = hello["nonce"]
        .as_str()
        .and_then(auth::unhex)
        .unwrap_or_default();
    let proof = hello["mac"].as_str().unwrap_or("");
    if cn.len() != 32 || !auth::verify(key, "client", &s, &cn, proof) {
        let _ = c.send(&json!({ "error": "not authenticated" }));
        return Err("not authenticated".into());
    }
    c.send(&json!({ "mac": auth::server_mac(key, &s, &cn) }))
        .map_err(|e| e.to_string())?;
    let session = hello["session"]
        .as_str()
        .map(|s| crate::policy::clip(s.trim(), 24))
        .filter(|s| !s.is_empty());
    Ok(Hello { session })
}

/// Client side: Ok when the server proved the key too.
pub fn client_handshake<R: Read, W: Write>(
    c: &mut Conn<R, W>,
    key: &[u8; 32],
    hello: &Hello,
) -> Result<(), String> {
    let first: Value = c
        .recv(4096)
        .map_err(|e| e.to_string())?
        .ok_or("closed before the handshake")?;
    if first["protocol"] != auth::PROTOCOL {
        return Err("not Acta's browser (wrong protocol)".into());
    }
    let s = first["nonce"]
        .as_str()
        .and_then(auth::unhex)
        .filter(|n| n.len() == 32)
        .ok_or("not Acta's browser (no nonce)")?;
    let cn = auth::nonce();
    c.send(&json!({
        "nonce": auth::hex(&cn),
        "mac": auth::client_mac(key, &s, &cn),
        "session": hello.session,
    }))
    .map_err(|e| e.to_string())?;
    let r: Value = c
        .recv(4096)
        .map_err(|e| e.to_string())?
        .ok_or("the app closed the connection during the handshake")?;
    if let Some(e) = r["error"].as_str() {
        return Err(format!(
            "the app refused this client: {}",
            crate::policy::clip(e, 60)
        ));
    }
    if !auth::verify(key, "server", &s, &cn, r["mac"].as_str().unwrap_or("")) {
        return Err("the pipe's server did not prove Acta's key: not the Acta app".into());
    }
    Ok(())
}
