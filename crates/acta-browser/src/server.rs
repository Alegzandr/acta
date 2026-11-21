//! The app's side of the pipe, transport aside: authenticate a connection,
//! then answer its requests one at a time. One connection controls the
//! browser at a time (ADR 0010, decision 3); another session's calls get one
//! line until the lease is free. A lease left idle for [`LEASE_IDLE`] can be
//! taken by another session, so a Claude Code session that went away never
//! blocks the browser for good.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::args::{Reply, Request};
use crate::ops::{self, Host};
use crate::wire::{self, Conn, REQUEST_MAX};

pub const IN_USE: &str = "the browser is in use by another Claude Code session";
pub const LEASE_IDLE: Duration = Duration::from_secs(300);

/// What the app shows about the connections: who drives the browser and
/// whether a call is running.
pub trait Status: Send + Sync {
    fn driver(&self, session: Option<&str>, connected: bool);
    fn busy(&self, busy: bool);
}

struct Lease {
    holder: u64,
    last: Instant,
    session: Option<String>,
}

pub struct Server<H: Host> {
    host: Arc<H>,
    status: Arc<dyn Status>,
    lease: Mutex<Option<Lease>>,
    next: AtomicU64,
}

/// Releases the lease when the connection ends, a panic included.
struct Held<'a, H: Host> {
    server: &'a Server<H>,
    id: u64,
}

impl<H: Host> Drop for Held<'_, H> {
    fn drop(&mut self) {
        let mut lease = self.server.lease.lock().unwrap_or_else(|p| p.into_inner());
        if lease.as_ref().is_some_and(|l| l.holder == self.id) {
            *lease = None;
            drop(lease);
            self.server.status.busy(false);
            self.server.status.driver(None, false);
        }
    }
}

impl<H: Host> Server<H> {
    pub fn new(host: Arc<H>, status: Arc<dyn Status>) -> Self {
        Server {
            host,
            status,
            lease: Mutex::new(None),
            next: AtomicU64::new(1),
        }
    }

    /// Takes or renews the lease for connection `id`; false when another
    /// connection holds it and used it recently.
    fn acquire(&self, id: u64, session: &Option<String>) -> bool {
        let mut lease = self.lease.lock().unwrap_or_else(|p| p.into_inner());
        let free = match lease.as_ref() {
            None => true,
            Some(l) => l.holder == id || l.last.elapsed() >= LEASE_IDLE,
        };
        if free {
            let changed = lease.as_ref().is_none_or(|l| l.holder != id);
            *lease = Some(Lease {
                holder: id,
                last: Instant::now(),
                session: session.clone(),
            });
            if changed {
                self.status.driver(session.as_deref(), true);
            }
        }
        free
    }

    fn touch(&self, id: u64) {
        let mut lease = self.lease.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(l) = lease.as_mut().filter(|l| l.holder == id) {
            l.last = Instant::now();
        }
    }

    /// The session holding the browser now.
    pub fn driver(&self) -> Option<String> {
        let lease = self.lease.lock().unwrap_or_else(|p| p.into_inner());
        lease.as_ref().and_then(|l| l.session.clone())
    }

    /// Serves one connection until it closes. `Err` only for a failed
    /// handshake, with the line to log.
    pub fn serve<R: Read, W: Write>(
        &self,
        mut conn: Conn<R, W>,
        key: &[u8; 32],
    ) -> Result<(), String> {
        let hello = wire::server_handshake(&mut conn, key)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let _held = Held { server: self, id };
        loop {
            let req: Result<Option<Request>, _> = conn.recv(REQUEST_MAX);
            let reply = match req {
                Ok(None) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    Reply::err("bad request: not one of the nine operations")
                }
                Err(_) => return Ok(()),
                Ok(Some(req)) => {
                    if !self.acquire(id, &hello.session) {
                        Reply::err(IN_USE)
                    } else {
                        self.status.busy(true);
                        let reply = ops::run(&*self.host, &req);
                        self.touch(id);
                        self.status.busy(false);
                        reply
                    }
                }
            };
            if conn.send(&reply).is_err() {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{ConsoleArgs, Request};
    use crate::ops::tests::{Fake, big_tree};
    use crate::wire::{Hello, REPLY_MAX};
    use std::io::{PipeReader, PipeWriter};

    #[derive(Default)]
    struct Seen(Mutex<Vec<String>>);

    impl Status for Seen {
        fn driver(&self, session: Option<&str>, connected: bool) {
            self.0
                .lock()
                .unwrap()
                .push(format!("{connected} {}", session.unwrap_or("-")));
        }
        fn busy(&self, _: bool) {}
    }

    type ClientConn = Conn<PipeReader, PipeWriter>;

    /// A client connected to `server` over in-memory pipes, served on its
    /// own thread.
    fn connect(
        server: &Arc<Server<Fake>>,
        server_key: [u8; 32],
    ) -> (ClientConn, std::thread::JoinHandle<Result<(), String>>) {
        let (cr, sw) = std::io::pipe().unwrap();
        let (sr, cw) = std::io::pipe().unwrap();
        let s = server.clone();
        let t = std::thread::spawn(move || s.serve(Conn::new(sr, sw), &server_key));
        (Conn::new(cr, cw), t)
    }

    fn console(c: &mut ClientConn) -> Reply {
        c.send(&Request::Console(ConsoleArgs::default())).unwrap();
        c.recv(REPLY_MAX).unwrap().unwrap()
    }

    fn server(seen: Arc<Seen>) -> Arc<Server<Fake>> {
        Arc::new(Server::new(Arc::new(Fake::new(big_tree(1))), seen))
    }

    #[test]
    fn a_client_with_the_wrong_key_is_refused_and_gets_nothing() {
        let seen = Arc::new(Seen::default());
        let s = server(seen.clone());
        let (mut c, t) = connect(&s, [1; 32]);
        let err = wire::client_handshake(&mut c, &[2; 32], &Hello::default()).unwrap_err();
        assert!(err.contains("refused"), "{err}");
        assert_eq!(t.join().unwrap().unwrap_err(), "not authenticated");
        assert!(seen.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_server_without_the_key_is_not_believed() {
        // The client side of a squatter: it answers the handshake without
        // knowing the key.
        let (cr, mut sw) = std::io::pipe().unwrap();
        let (mut sr, cw) = std::io::pipe().unwrap();
        let fake = std::thread::spawn(move || {
            use std::io::{BufRead, Write};
            writeln!(
                sw,
                r#"{{"protocol":"{}","nonce":"{}"}}"#,
                crate::auth::PROTOCOL,
                "ab".repeat(32)
            )
            .unwrap();
            let mut line = String::new();
            std::io::BufReader::new(&mut sr)
                .read_line(&mut line)
                .unwrap();
            writeln!(sw, r#"{{"mac":"{}"}}"#, "00".repeat(32)).unwrap();
        });
        let mut c = Conn::new(cr, cw);
        let err = wire::client_handshake(&mut c, &[1; 32], &Hello::default()).unwrap_err();
        assert!(err.contains("not the Acta app"), "{err}");
        fake.join().unwrap();
    }

    #[test]
    fn one_session_drives_at_a_time() {
        let seen = Arc::new(Seen::default());
        let s = server(seen.clone());
        let key = [9; 32];
        let (mut a, ta) = connect(&s, key);
        let first = Hello {
            session: Some("T-42".into()),
        };
        wire::client_handshake(&mut a, &key, &first).unwrap();
        let (mut b, tb) = connect(&s, key);
        wire::client_handshake(&mut b, &key, &Hello::default()).unwrap();
        assert!(console(&mut a).ok);
        assert_eq!(s.driver().as_deref(), Some("T-42"));
        let refused = console(&mut b);
        assert_eq!((refused.ok, refused.text.as_str()), (false, IN_USE));
        drop(a);
        ta.join().unwrap().unwrap();
        assert!(console(&mut b).ok);
        drop(b);
        tb.join().unwrap().unwrap();
        assert_eq!(
            *seen.0.lock().unwrap(),
            ["true T-42", "false -", "true -", "false -"]
        );
    }

    #[test]
    fn a_line_that_is_not_an_operation_is_answered_not_run() {
        let s = server(Arc::new(Seen::default()));
        let key = [3; 32];
        let (mut c, _t) = connect(&s, key);
        wire::client_handshake(&mut c, &key, &Hello::default()).unwrap();
        c.send(&serde_json::json!({"op": "eval", "args": {"script": "document.cookie"}}))
            .unwrap();
        let r: Reply = c.recv(REPLY_MAX).unwrap().unwrap();
        assert!(!r.ok && r.text.starts_with("bad request"));
        assert!(console(&mut c).ok);
    }
}
