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
/// The longest a request line may be; longer ones are dropped whole.
const LINE_MAX: u64 = REQUEST_MAX;

/// What the app shows about the connections: who drives the browser and
/// whether a call is running.
pub trait Status: Send + Sync {
    fn driver(&self, session: Option<&str>, connected: bool);
    fn busy(&self, busy: bool);
}

struct Lease {
    holder: u64,
    last: Instant,
    /// A call of the holder is running (a handoff waits up to 10 min): a
    /// busy lease is never idle.
    busy: bool,
    session: Option<String>,
}

pub struct Server<H: Host> {
    host: Arc<H>,
    status: Arc<dyn Status>,
    lease: Mutex<Option<Lease>>,
    next: AtomicU64,
    idle: Duration,
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
            idle: LEASE_IDLE,
        }
    }

    /// Tests: a shorter idle time before another session may take over.
    pub fn with_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Takes or renews the lease for connection `id` and marks it busy;
    /// false when another connection holds it and is running a call or
    /// used it recently.
    fn acquire(&self, id: u64, session: &Option<String>) -> bool {
        let mut lease = self.lease.lock().unwrap_or_else(|p| p.into_inner());
        let free = match lease.as_ref() {
            None => true,
            Some(l) => l.holder == id || (!l.busy && l.last.elapsed() >= self.idle),
        };
        if free {
            let changed = lease.as_ref().is_none_or(|l| l.holder != id);
            *lease = Some(Lease {
                holder: id,
                last: Instant::now(),
                busy: true,
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
            l.busy = false;
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
            let req: Result<Option<Request>, _> = conn.recv(LINE_MAX);
            let reply = match req {
                Ok(None) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    Reply::err("bad request: not one of the nine operations")
                }
                Err(_) => return Ok(()),
                Ok(Some(req)) => {
                    if let Err(why) = req.check() {
                        Reply::err(why)
                    } else if !self.acquire(id, &hello.session) {
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
    fn a_session_in_a_long_call_keeps_the_browser() {
        let mut fake = Fake::new(big_tree(1));
        fake.handoff_delay = Duration::from_millis(600);
        let s = Arc::new(
            Server::new(Arc::new(fake), Arc::new(Seen::default()))
                .with_idle(Duration::from_millis(50)),
        );
        let key = [4; 32];
        let (mut a, _ta) = connect(&s, key);
        wire::client_handshake(&mut a, &key, &Hello::default()).unwrap();
        let (mut b, _tb) = connect(&s, key);
        wire::client_handshake(&mut b, &key, &Hello::default()).unwrap();
        assert!(console(&mut a).ok);
        a.send(&Request::Handoff(crate::args::HandoffArgs {
            message: "Sign in, then press Done.".into(),
        }))
        .unwrap();
        // Well past the idle time, the handoff still runs: not taken over.
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(console(&mut b).text, IN_USE);
        let done: Reply = a.recv(REPLY_MAX).unwrap().unwrap();
        assert!(done.ok, "{}", done.text);
        // Idle once the call ended: the other session may take it.
        std::thread::sleep(Duration::from_millis(100));
        assert!(console(&mut b).ok);
    }

    #[test]
    fn an_oversized_line_is_refused_and_the_next_request_answered_in_step() {
        use std::io::Write;
        let s = server(Arc::new(Seen::default()));
        let key = [5; 32];
        let (cr, sw) = std::io::pipe().unwrap();
        let (sr, mut cw) = std::io::pipe().unwrap();
        let srv = s.clone();
        let _t = std::thread::spawn(move || srv.serve(Conn::new(sr, sw), &key));
        let writer = cw.try_clone().unwrap();
        let mut c = Conn::new(cr, writer);
        wire::client_handshake(&mut c, &key, &Hello::default()).unwrap();
        let big = format!(
            "{{\"op\":\"type\",\"args\":{{\"ref\":\"e2\",\"text\":\"{}\"}}}}\n",
            "a".repeat(200_000)
        );
        cw.write_all(big.as_bytes()).unwrap();
        let first: Reply = c.recv(REPLY_MAX).unwrap().unwrap();
        assert!(
            !first.ok && first.text.starts_with("bad request"),
            "{}",
            first.text
        );
        let second = console(&mut c);
        assert!(
            second.ok && second.text.starts_with("console"),
            "{}",
            second.text
        );
        // A line within the limit but over a field limit: refused, in step.
        c.send(&Request::Type(crate::args::TypeArgs {
            target: "e2".into(),
            text: Some("a".repeat(3_000)),
            ..Default::default()
        }))
        .unwrap();
        let third: Reply = c.recv(REPLY_MAX).unwrap().unwrap();
        assert!(third.text.contains("at most 2000"), "{}", third.text);
        assert!(console(&mut c).ok);
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
