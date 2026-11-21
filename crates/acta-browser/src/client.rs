//! `acta mcp browser`'s side of the pipe: connect, authenticate, call.

use std::fs::File;
use std::time::Duration;

use crate::args::{Reply, Request};
use crate::wire::{self, Conn, Hello, REPLY_MAX};

/// Why no connection was made.
#[derive(Debug)]
pub enum ConnectError {
    /// Nobody serves the pipe: the app is not running.
    NoApp,
    /// Something answered and failed the handshake, or I/O failed.
    Failed(String),
}

pub struct Client {
    conn: Conn<File, File>,
}

impl Client {
    pub fn connect(name: &str, key: &[u8; 32], hello: &Hello) -> Result<Client, ConnectError> {
        let file = crate::pipe::connect(name, Duration::from_secs(2)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ConnectError::NoApp
            } else {
                ConnectError::Failed(format!("cannot open the pipe: {}", e.kind()))
            }
        })?;
        let writer = file
            .try_clone()
            .map_err(|e| ConnectError::Failed(e.to_string()))?;
        let mut conn = Conn::new(file, writer);
        wire::client_handshake(&mut conn, key, hello).map_err(ConnectError::Failed)?;
        Ok(Client { conn })
    }

    /// One request and its reply. `Err` when the connection is gone (the
    /// app closed).
    pub fn call(&mut self, req: &Request) -> std::io::Result<Reply> {
        self.conn.send(req)?;
        self.conn
            .recv(REPLY_MAX)?
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"))
    }
}
