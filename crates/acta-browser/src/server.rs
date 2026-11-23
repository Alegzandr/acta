//! The app's side of the pipe, transport aside: authenticate a connection,
//! then answer its requests one at a time. Browsers belong to pages
//! (docs/design/layout.md 4.5.1): a session drives the browsers of its own
//! page only, its own by default (opened on its first call), another one of
//! the page by number; every reply's first line lists the page's browsers.
//! One connection controls a browser at a time (ADR 0010, decision 3);
//! another session's calls to it get one line until the lease is free. A
//! lease left idle for [`LEASE_IDLE`] can be taken by another session, so
//! a Claude Code session that went away never blocks a browser for good.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::args::{Reply, Request};
use crate::ops::{self, Host};
use crate::wire::{self, Conn, Hello, REQUEST_MAX};

pub const IN_USE: &str = "the browser is in use by another Claude Code session";
/// A session Acta cannot place on a page has no browser.
pub const NO_PAGE: &str =
    "no Acta page for this session: start Claude Code from a project, epic or topic in Acta";
pub const LEASE_IDLE: Duration = Duration::from_secs(300);
/// The longest a request line may be; longer ones are dropped whole.
const LINE_MAX: u64 = REQUEST_MAX;

/// A browser of a page: its number and who opened it (the session's
/// identity, see [`Hello::identity`]; `None` for one the human opened).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserOf {
    pub n: u8,
    pub opener: Option<String>,
}

/// The app's browsers, by page (`T-42`, `E-3`, `P-1`) and number, and what
/// it shows of them: who drives each and whether a call runs.
pub trait Pages: Send + Sync {
    type Host: Host;
    /// The page's browsers, in number order.
    fn browsers(&self, page: &str) -> Vec<BrowserOf>;
    fn host(&self, page: &str, n: u8) -> Option<Arc<Self::Host>>;
    /// Opens a new browser on `page` for the session `opener` (its launch
    /// token names it after the session: "Browser for Claude 2"); `Err`
    /// is one line (the page's limit).
    fn open(&self, page: &str, opener: &str, launch: Option<&str>) -> Result<u8, String>;
    fn driver(&self, page: &str, n: u8, session: Option<&str>, connected: bool);
    fn busy(&self, page: &str, n: u8, busy: bool);
}

/// "browsers: 2 (yours), 1": the calling session's own first.
pub fn browsers_line(list: &[BrowserOf], me: &str) -> String {
    let mut mine: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    for b in list {
        if b.opener.as_deref() == Some(me) {
            mine.push(format!("{} (yours)", b.n));
        } else {
            others.push(b.n.to_string());
        }
    }
    mine.append(&mut others);
    if mine.is_empty() {
        "browsers: none".into()
    } else {
        format!("browsers: {}", mine.join(", "))
    }
}

struct Lease {
    holder: u64,
    last: Instant,
    /// A call of the holder is running (a handoff waits up to 10 min): a
    /// busy lease is never idle.
    busy: bool,
    session: Option<String>,
}

pub struct Server<P: Pages> {
    pages: Arc<P>,
    /// One lease per browser, by page and number.
    leases: Mutex<HashMap<(String, u8), Lease>>,
    next: AtomicU64,
    idle: Duration,
}

/// Releases the connection's leases when it ends, a panic included.
struct Held<'a, P: Pages> {
    server: &'a Server<P>,
    id: u64,
}

impl<P: Pages> Drop for Held<'_, P> {
    fn drop(&mut self) {
        let mut leases = self.server.leases.lock().unwrap_or_else(|p| p.into_inner());
        let mine: Vec<(String, u8)> = leases
            .iter()
            .filter(|(_, l)| l.holder == self.id)
            .map(|(k, _)| k.clone())
            .collect();
        for key in &mine {
            leases.remove(key);
        }
        drop(leases);
        for (page, n) in mine {
            self.server.pages.busy(&page, n, false);
            self.server.pages.driver(&page, n, None, false);
        }
    }
}

impl<P: Pages> Server<P> {
    pub fn new(pages: Arc<P>) -> Self {
        Server {
            pages,
            leases: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            idle: LEASE_IDLE,
        }
    }

    /// Tests: a shorter idle time before another session may take over.
    pub fn with_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Takes or renews the lease of browser `(page, n)` for connection
    /// `id` and marks it busy; false when another connection holds it and
    /// is running a call or used it recently.
    fn acquire(&self, id: u64, page: &str, n: u8, session: &Option<String>) -> bool {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        let key = (page.to_owned(), n);
        let free = match leases.get(&key) {
            None => true,
            Some(l) => l.holder == id || (!l.busy && l.last.elapsed() >= self.idle),
        };
        if free {
            let changed = leases.get(&key).is_none_or(|l| l.holder != id);
            leases.insert(
                key,
                Lease {
                    holder: id,
                    last: Instant::now(),
                    busy: true,
                    session: session.clone(),
                },
            );
            drop(leases);
            if changed {
                self.pages.driver(page, n, session.as_deref(), true);
            }
        }
        free
    }

    fn touch(&self, id: u64, page: &str, n: u8) {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(l) = leases
            .get_mut(&(page.to_owned(), n))
            .filter(|l| l.holder == id)
        {
            l.last = Instant::now();
            l.busy = false;
        }
    }

    /// The session driving browser `(page, n)` now.
    pub fn driver(&self, page: &str, n: u8) -> Option<String> {
        let leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        leases
            .get(&(page.to_owned(), n))
            .and_then(|l| l.session.clone())
    }

    /// The browser a request goes to: the one it names, else the
    /// session's own, opened on first use. `Err` is the one-line reply.
    fn target(
        &self,
        hello: &Hello,
        me: &str,
        named: Option<u8>,
    ) -> Result<(String, u8, Arc<P::Host>), String> {
        let page = hello.page.clone().ok_or(NO_PAGE)?;
        let list = self.pages.browsers(&page);
        let n = match named {
            Some(n) if list.iter().any(|b| b.n == n) => n,
            Some(n) => {
                return Err(format!(
                    "no browser {n} on this page; {}",
                    browsers_line(&list, me)
                ));
            }
            None => match list.iter().find(|b| b.opener.as_deref() == Some(me)) {
                Some(b) => b.n,
                None => self.pages.open(&page, me, hello.launch.as_deref())?,
            },
        };
        let host = self
            .pages
            .host(&page, n)
            .ok_or_else(|| format!("browser {n} was closed; call again"))?;
        Ok((page, n, host))
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
        // The session's identity: its launch token when Acta started it,
        // else this connection.
        let me = hello.identity(id);
        let _held = Held { server: self, id };
        loop {
            let req: Result<Option<Request>, _> = conn.recv(LINE_MAX);
            let reply = match req {
                Ok(None) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    Reply::err("bad request: not one of the nine operations")
                }
                Err(_) => return Ok(()),
                Ok(Some(req)) => match req.check() {
                    Err(why) => Reply::err(why),
                    Ok(()) => match self.target(&hello, &me, req.browser()) {
                        Err(line) => Reply::err(line),
                        Ok((page, n, host)) => {
                            if !self.acquire(id, &page, n, &hello.session) {
                                Reply::err(IN_USE)
                            } else {
                                self.pages.busy(&page, n, true);
                                let mut reply = ops::run(&*host, &req);
                                self.touch(id, &page, n);
                                self.pages.busy(&page, n, false);
                                let line = browsers_line(&self.pages.browsers(&page), &me);
                                reply.text = format!("{line}\n{}", reply.text);
                                reply
                            }
                        }
                    },
                },
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
    use crate::wire::REPLY_MAX;
    use std::io::{PipeReader, PipeWriter};

    type PageBrowsers = Vec<(BrowserOf, Arc<Fake>)>;

    /// The app's pages, in memory: browsers by page, opened on demand up
    /// to 3 a page, and what the header was told.
    #[derive(Default)]
    struct FakePages {
        browsers: Mutex<HashMap<String, PageBrowsers>>,
        seen: Mutex<Vec<String>>,
        handoff_delay: Option<Duration>,
    }

    impl FakePages {
        fn human_opens(&self, page: &str, n: u8) {
            self.browsers
                .lock()
                .unwrap()
                .entry(page.into())
                .or_default()
                .push((
                    BrowserOf { n, opener: None },
                    Arc::new(Fake::new(big_tree(1))),
                ));
        }
    }

    impl Pages for FakePages {
        type Host = Fake;
        fn browsers(&self, page: &str) -> Vec<BrowserOf> {
            let map = self.browsers.lock().unwrap();
            let mut list: Vec<BrowserOf> = map
                .get(page)
                .map(|v| v.iter().map(|(b, _)| b.clone()).collect())
                .unwrap_or_default();
            list.sort_by_key(|b| b.n);
            list
        }
        fn host(&self, page: &str, n: u8) -> Option<Arc<Fake>> {
            let map = self.browsers.lock().unwrap();
            map.get(page)?
                .iter()
                .find(|(b, _)| b.n == n)
                .map(|(_, h)| h.clone())
        }
        fn open(&self, page: &str, opener: &str, _: Option<&str>) -> Result<u8, String> {
            let mut map = self.browsers.lock().unwrap();
            let list = map.entry(page.into()).or_default();
            if list.len() >= 3 {
                return Err("3 is the most on one page".into());
            }
            let n = (1..)
                .find(|n| !list.iter().any(|(b, _)| b.n == *n))
                .unwrap();
            let mut fake = Fake::new(big_tree(1));
            if let Some(d) = self.handoff_delay {
                fake.handoff_delay = d;
            }
            list.push((
                BrowserOf {
                    n,
                    opener: Some(opener.into()),
                },
                Arc::new(fake),
            ));
            Ok(n)
        }
        fn driver(&self, page: &str, n: u8, session: Option<&str>, connected: bool) {
            self.seen
                .lock()
                .unwrap()
                .push(format!("{page}/{n} {connected} {}", session.unwrap_or("-")));
        }
        fn busy(&self, _: &str, _: u8, _: bool) {}
    }

    type ClientConn = Conn<PipeReader, PipeWriter>;

    fn connect(
        server: &Arc<Server<FakePages>>,
        key: [u8; 32],
        hello: Hello,
    ) -> (ClientConn, std::thread::JoinHandle<Result<(), String>>) {
        let (cr, sw) = std::io::pipe().unwrap();
        let (sr, cw) = std::io::pipe().unwrap();
        let s = server.clone();
        let t = std::thread::spawn(move || s.serve(Conn::new(sr, sw), &key));
        let mut c = Conn::new(cr, cw);
        wire::client_handshake(&mut c, &key, &hello).unwrap();
        (c, t)
    }

    fn on(page: &str, launch: &str, session: &str) -> Hello {
        Hello {
            session: Some(session.into()),
            page: Some(page.into()),
            launch: Some(launch.into()),
        }
    }

    fn console(c: &mut ClientConn, browser: Option<u8>) -> Reply {
        c.send(&Request::Console(ConsoleArgs {
            browser,
            ..Default::default()
        }))
        .unwrap();
        c.recv(REPLY_MAX).unwrap().unwrap()
    }

    fn server(pages: FakePages) -> Arc<Server<FakePages>> {
        Arc::new(Server::new(Arc::new(pages)))
    }

    #[test]
    fn two_sessions_of_a_page_each_drive_their_own_browser() {
        let s = server(FakePages::default());
        let key = [9; 32];
        let (mut a, _ta) = connect(&s, key, on("T-42", "tok-a", "T-42"));
        let (mut b, _tb) = connect(&s, key, on("T-42", "tok-b", "T-42"));
        let ra = console(&mut a, None);
        assert!(ra.ok, "{}", ra.text);
        assert_eq!(ra.text.lines().next(), Some("browsers: 1 (yours)"));
        // The second session gets a browser of its own, at the same time.
        let rb = console(&mut b, None);
        assert!(rb.ok, "{}", rb.text);
        assert_eq!(rb.text.lines().next(), Some("browsers: 2 (yours), 1"));
        assert_eq!(
            console(&mut a, None).text.lines().next(),
            Some("browsers: 1 (yours), 2")
        );
        assert_eq!(s.driver("T-42", 1).as_deref(), Some("T-42"));
        assert_eq!(s.driver("T-42", 2).as_deref(), Some("T-42"));
    }

    #[test]
    fn a_browser_of_the_page_is_driven_by_number_and_another_page_is_out_of_reach() {
        let pages = FakePages::default();
        pages.human_opens("T-42", 1);
        pages.human_opens("T-51", 1);
        let s = server(pages);
        let key = [7; 32];
        let (mut a, _t) = connect(&s, key, on("T-42", "tok-a", "T-42"));
        let r = console(&mut a, Some(1));
        assert!(r.ok, "{}", r.text);
        assert_eq!(r.text.lines().next(), Some("browsers: 1"));
        // Numbers are the page's own: 2 is nothing here, whatever T-51 has.
        let r = console(&mut a, Some(2));
        assert!(!r.ok);
        assert_eq!(r.text, "no browser 2 on this page; browsers: 1");
        assert_eq!(r.text.lines().count(), 1, "one line");
        // A session with no page gets one line too.
        let (mut lost, _t2) = connect(&s, key, Hello::default());
        let r = console(&mut lost, None);
        assert_eq!((r.ok, r.text.as_str()), (false, NO_PAGE));
    }

    #[test]
    fn the_page_limit_answers_in_one_line() {
        let s = server(FakePages::default());
        let key = [6; 32];
        let mut conns = Vec::new();
        for i in 0..3 {
            let (mut c, t) = connect(&s, key, on("E-3", &format!("tok-{i}"), "E-3"));
            assert!(console(&mut c, None).ok);
            conns.push((c, t));
        }
        let (mut d, _t) = connect(&s, key, on("E-3", "tok-d", "E-3"));
        let r = console(&mut d, None);
        assert!(!r.ok && !r.text.contains('\n'), "{}", r.text);
    }

    #[test]
    fn a_session_in_a_long_call_keeps_the_browser() {
        let pages = FakePages {
            handoff_delay: Some(Duration::from_millis(600)),
            ..Default::default()
        };
        let s = Arc::new(Server::new(Arc::new(pages)).with_idle(Duration::from_millis(50)));
        let key = [4; 32];
        let (mut a, _ta) = connect(&s, key, on("T-1", "tok-a", "T-1"));
        let (mut b, _tb) = connect(&s, key, on("T-1", "tok-b", "T-1"));
        assert!(console(&mut a, None).ok);
        a.send(&Request::Handoff(crate::args::HandoffArgs {
            message: "Sign in, then press Done.".into(),
            browser: None,
        }))
        .unwrap();
        // Well past the idle time, the handoff still runs: not taken over.
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(console(&mut b, Some(1)).text, IN_USE);
        let done: Reply = a.recv(REPLY_MAX).unwrap().unwrap();
        assert!(done.ok, "{}", done.text);
        // Idle once the call ended: the other session may take it.
        std::thread::sleep(Duration::from_millis(100));
        assert!(console(&mut b, Some(1)).ok);
    }

    #[test]
    fn one_session_drives_a_browser_at_a_time() {
        let pages = FakePages::default();
        pages.human_opens("T-42", 1);
        let s = server(pages);
        let key = [9; 32];
        let (mut a, ta) = connect(&s, key, on("T-42", "tok-a", "T-42"));
        let (mut b, tb) = connect(&s, key, on("T-42", "tok-b", "T-42"));
        assert!(console(&mut a, Some(1)).ok);
        let refused = console(&mut b, Some(1));
        assert_eq!((refused.ok, refused.text.as_str()), (false, IN_USE));
        drop(a);
        ta.join().unwrap().unwrap();
        assert!(console(&mut b, Some(1)).ok);
        drop(b);
        tb.join().unwrap().unwrap();
        assert_eq!(
            *s.pages.seen.lock().unwrap(),
            [
                "T-42/1 true T-42",
                "T-42/1 false -",
                "T-42/1 true T-42",
                "T-42/1 false -"
            ]
        );
    }

    #[test]
    fn an_oversized_line_is_refused_and_the_next_request_answered_in_step() {
        let s = server(FakePages::default());
        let key = [5; 32];
        let (cr, sw) = std::io::pipe().unwrap();
        let (sr, mut cw) = std::io::pipe().unwrap();
        let srv = s.clone();
        let _t = std::thread::spawn(move || srv.serve(Conn::new(sr, sw), &key));
        let writer = cw.try_clone().unwrap();
        let mut c = Conn::new(cr, writer);
        wire::client_handshake(&mut c, &key, &on("T-1", "tok", "T-1")).unwrap();
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
        let second = console(&mut c, None);
        assert!(second.ok, "{}", second.text);
        // A line within the limit but over a field limit: refused, in step.
        c.send(&Request::Type(crate::args::TypeArgs {
            target: "e2".into(),
            text: Some("a".repeat(3_000)),
            ..Default::default()
        }))
        .unwrap();
        let third: Reply = c.recv(REPLY_MAX).unwrap().unwrap();
        assert!(third.text.contains("at most 2000"), "{}", third.text);
        assert!(console(&mut c, None).ok);
    }

    #[test]
    fn a_client_with_the_wrong_key_is_refused_and_gets_nothing() {
        let s = server(FakePages::default());
        let (cr, sw) = std::io::pipe().unwrap();
        let (sr, cw) = std::io::pipe().unwrap();
        let srv = s.clone();
        let t = std::thread::spawn(move || srv.serve(Conn::new(sr, sw), &[1; 32]));
        let mut c = Conn::new(cr, cw);
        let err = wire::client_handshake(&mut c, &[2; 32], &Hello::default()).unwrap_err();
        assert!(err.contains("refused"), "{err}");
        assert_eq!(t.join().unwrap().unwrap_err(), "not authenticated");
        assert!(s.pages.seen.lock().unwrap().is_empty());
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
    fn a_line_that_is_not_an_operation_is_answered_not_run() {
        let s = server(FakePages::default());
        let key = [3; 32];
        let (mut c, _t) = connect(&s, key, on("T-1", "tok", "T-1"));
        c.send(&serde_json::json!({"op": "eval", "args": {"script": "document.cookie"}}))
            .unwrap();
        let r: Reply = c.recv(REPLY_MAX).unwrap().unwrap();
        assert!(!r.ok && r.text.starts_with("bad request"));
        assert!(console(&mut c, None).ok);
    }

    #[test]
    fn the_browsers_line_puts_the_callers_own_first() {
        let list = [
            BrowserOf { n: 1, opener: None },
            BrowserOf {
                n: 2,
                opener: Some("me".into()),
            },
            BrowserOf {
                n: 3,
                opener: Some("other".into()),
            },
        ];
        assert_eq!(browsers_line(&list, "me"), "browsers: 2 (yours), 1, 3");
        assert_eq!(browsers_line(&[], "me"), "browsers: none");
    }
}
