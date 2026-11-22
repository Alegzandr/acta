//! The Whisper worker process (ADR 0014, 5 and its "embedded only"
//! addendum): the GUI executable started with [`WORKER_ARG`] and nothing
//! else, speaking the [`protocol`](super::protocol) over its stdin and
//! stdout. Both sides live here so they share one definition:
//!
//! - [`serve`]: the worker's loop, generic over how an engine is made (the
//!   app makes a Whisper engine; tests a fake one).
//! - [`WorkerClient`]: the app's side, an [`Engine`] that starts the worker
//!   when a take starts, restarts it when it has exited, and ties it to
//!   Acta's lifetime: a job object with kill-on-close on Windows, the
//!   parent-death signal on Linux, and the end of its stdin everywhere (the
//!   pipe closes when Acta exits), so it never outlives Acta. Its stderr is
//!   discarded.

use std::collections::VecDeque;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zeroize::Zeroize;

use super::engine::{Engine, EngineError, Lang};
use super::protocol::{
    self, EngineSpec, IDLE_EXIT, Init, Reply, Request, WORKER_ARG, read_reply, write_reply,
};
use crate::process::{Tree, spawn_in_tree};

/// How [`serve`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServeEnd {
    /// The app closed the pipe (it quit, or let the worker go).
    Closed,
    /// No request for the idle time.
    Idle,
    /// The engine stopped for good; the process should exit with
    /// [`protocol::EXIT_ENGINE_GONE`].
    EngineGone,
    /// A malformed frame, or the reply pipe broke.
    Broken,
}

/// The worker's loop. `make` builds the engine from the `Init` request.
/// Requests are read on a thread of their own so the idle clock runs while
/// the pipe is quiet.
pub fn serve<R, W, F>(input: R, output: W, mut make: F) -> ServeEnd
where
    R: Read + Send + 'static,
    W: Write,
    F: FnMut(&EngineSpec) -> Result<Box<dyn Engine>, EngineError>,
{
    let (tx, rx) = mpsc::sync_channel::<io::Result<Option<Request>>>(1);
    let reader = std::thread::Builder::new()
        .name("acta-voice-requests".into())
        .spawn(move || {
            let mut input = BufReader::new(input);
            loop {
                let req = protocol::read_request(&mut input);
                let last = !matches!(req, Ok(Some(_)));
                if tx.send(req).is_err() || last {
                    return;
                }
            }
        });
    if reader.is_err() {
        return ServeEnd::Broken;
    }
    let mut out = BufWriter::new(output);
    let mut idle = IDLE_EXIT;
    let mut engine: Option<Box<dyn Engine>> = None;
    loop {
        let req = match rx.recv_timeout(idle) {
            Ok(Ok(Some(req))) => req,
            Ok(Ok(None)) | Err(mpsc::RecvTimeoutError::Disconnected) => return ServeEnd::Closed,
            Ok(Err(_)) => return ServeEnd::Broken,
            Err(mpsc::RecvTimeoutError::Timeout) => return ServeEnd::Idle,
        };
        let reply = match req {
            Request::Init(Init { spec, idle_secs }) => {
                idle = Duration::from_secs(idle_secs.max(1));
                match make(&spec) {
                    Ok(e) => {
                        engine = Some(e);
                        Some(Reply::Ready)
                    }
                    Err(e) => Some(Reply::Error(e)),
                }
            }
            Request::Start(lang) => Some(match engine.as_mut() {
                None => Reply::Error(EngineError::Model),
                Some(e) => match e.start(lang) {
                    Ok(()) => Reply::Started,
                    Err(err) => Reply::Error(err),
                },
            }),
            Request::Phrase(mut samples) => {
                let reply = match engine.as_mut() {
                    None => Reply::Error(EngineError::Model),
                    Some(e) => match e.feed(samples.as_slice()) {
                        Ok(text) => Reply::Text(text),
                        Err(err) => Reply::Error(err),
                    },
                };
                samples.wipe();
                Some(reply)
            }
            Request::Stop => {
                if let Some(e) = engine.as_mut() {
                    e.stop();
                }
                None
            }
        };
        let Some(mut reply) = reply else {
            continue;
        };
        let gone = reply == Reply::Error(EngineError::Exited);
        let written = write_reply(&mut out, &reply);
        if let Reply::Text(t) = &mut reply {
            t.zeroize();
        }
        if gone {
            return ServeEnd::EngineGone;
        }
        if written.is_err() {
            return ServeEnd::Broken;
        }
    }
}

/// The command that starts the worker: the executable and [`WORKER_ARG`],
/// nothing else (no data in process arguments, ADR 0014, 7).
pub fn worker_command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // whisper.cpp and the runtime may print there; nobody reads it.
        .stderr(Stdio::null());
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: async-signal-safe calls only, between fork and exec.
        unsafe {
            command.pre_exec(|| {
                // Killed when the thread that started it ends (the voice
                // engine thread, which lives as long as the app's
                // controller); stdin EOF covers the rest.
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
    }
    command
}

struct Running {
    child: Child,
    tree: Tree,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// Replies owed for requests already sent, before the next `Text`.
    owed: VecDeque<Owed>,
    /// The last request sent: a worker close to its idle exit is replaced
    /// rather than written to as it leaves.
    last_sent: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owed {
    Ready,
    Started,
}

impl Running {
    fn send(&mut self, req: &Request) -> io::Result<()> {
        self.last_sent = Instant::now();
        protocol::write_request(&mut self.stdin, req)
    }

    /// Reads the replies owed, then one more (the phrase's).
    fn settle(&mut self) -> Result<Reply, EngineError> {
        while let Some(owed) = self.owed.pop_front() {
            match read_reply(&mut self.stdout) {
                Ok(Some(Reply::Ready)) if owed == Owed::Ready => {}
                Ok(Some(Reply::Started)) if owed == Owed::Started => {}
                Ok(Some(Reply::Error(e))) => {
                    // A failed load or start: nothing more is coming for
                    // the requests behind it.
                    self.owed.clear();
                    return Err(e);
                }
                _ => return Err(EngineError::Exited),
            }
        }
        match read_reply(&mut self.stdout) {
            Ok(Some(reply)) => Ok(reply),
            _ => Err(EngineError::Exited),
        }
    }

    fn end(mut self) {
        self.tree.kill(&mut self.child);
        let _ = self.child.wait();
    }
}

/// The app's side of the worker, as an [`Engine`]. One per app, used from
/// the voice engine thread only.
pub struct WorkerClient {
    exe: PathBuf,
    init: Init,
    running: Option<Running>,
    /// The worker failed to load the model: later takes report it at once
    /// until the spec changes.
    model_failed: bool,
}

impl WorkerClient {
    /// `exe` is the GUI executable (`std::env::current_exe()`).
    pub fn new(exe: PathBuf, spec: EngineSpec) -> Self {
        WorkerClient {
            exe,
            init: Init {
                spec,
                idle_secs: IDLE_EXIT.as_secs(),
            },
            running: None,
            model_failed: false,
        }
    }

    /// Shorter idle exit, for tests.
    pub fn with_idle(mut self, idle: Duration) -> Self {
        self.init.idle_secs = idle.as_secs().max(1);
        self
    }

    /// The worker's process id, while one runs.
    pub fn pid(&self) -> Option<u32> {
        self.running.as_ref().map(|r| r.child.id())
    }

    fn spawn(&self) -> io::Result<Running> {
        let mut command = worker_command(&self.exe);
        let (mut child, tree) = spawn_in_tree(&mut command)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            tree.kill(&mut child);
            let _ = child.wait();
            return Err(io::ErrorKind::BrokenPipe.into());
        };
        let mut running = Running {
            child,
            tree,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            owed: VecDeque::new(),
            last_sent: Instant::now(),
        };
        running.send(&Request::Init(self.init.clone()))?;
        running.owed.push_back(Owed::Ready);
        Ok(running)
    }

    /// The running worker, started again if it exited (idle, crash).
    fn alive(&mut self) -> Result<&mut Running, EngineError> {
        let idle = Duration::from_secs(self.init.idle_secs);
        let margin = (idle / 10).min(Duration::from_secs(15));
        let exited = match self.running.as_mut() {
            Some(r) => {
                !matches!(r.child.try_wait(), Ok(None)) || r.last_sent.elapsed() + margin >= idle
            }
            None => true,
        };
        if exited {
            if let Some(r) = self.running.take() {
                r.end();
            }
            self.running = Some(self.spawn().map_err(|_| EngineError::Exited)?);
        }
        self.running.as_mut().ok_or(EngineError::Exited)
    }

    fn fail(&mut self, e: EngineError) -> EngineError {
        if e == EngineError::Model {
            self.model_failed = true;
        }
        if e != EngineError::Failed
            && let Some(r) = self.running.take()
        {
            r.end();
        }
        e
    }
}

impl Engine for WorkerClient {
    /// Sends the start without waiting: the model loads while the person
    /// speaks the first words. A worker that exited is started again.
    fn start(&mut self, lang: Lang) -> Result<(), EngineError> {
        if self.model_failed {
            return Err(EngineError::Model);
        }
        let first = self.alive().and_then(|r| {
            r.send(&Request::Start(lang))
                .map_err(|_| EngineError::Exited)?;
            r.owed.push_back(Owed::Started);
            Ok(())
        });
        match first {
            Ok(()) => Ok(()),
            // The worker had just idled out: once more with a new one.
            Err(_) => {
                if let Some(r) = self.running.take() {
                    r.end();
                }
                let r = self.alive()?;
                r.send(&Request::Start(lang))
                    .map_err(|_| EngineError::Exited)?;
                r.owed.push_back(Owed::Started);
                Ok(())
            }
        }
    }

    fn feed(&mut self, phrase: &[f32]) -> Result<String, EngineError> {
        let Some(r) = self.running.as_mut() else {
            return Err(EngineError::Exited);
        };
        r.last_sent = Instant::now();
        if protocol::write_phrase(&mut r.stdin, phrase).is_err() {
            return Err(self.fail(EngineError::Exited));
        }
        match r.settle() {
            Ok(Reply::Text(t)) => Ok(t),
            Ok(Reply::Error(e)) | Err(e) => Err(self.fail(e)),
            Ok(_) => Err(self.fail(EngineError::Exited)),
        }
    }

    fn stop(&mut self) {
        if let Some(r) = self.running.as_mut() {
            let _ = r.send(&Request::Stop);
        }
    }

    /// Ends the worker (and with it any hold on the model file); a model
    /// that failed to load is tried again at the next start.
    fn release(&mut self) {
        if let Some(r) = self.running.take() {
            r.end();
        }
        self.model_failed = false;
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        if let Some(r) = self.running.take() {
            r.end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::buffer::AudioBuf;
    use crate::voice::engine::{FakeEngine, FakeStep};
    use crate::voice::protocol::{read_reply, write_request};

    fn fake_maker(spec: &EngineSpec) -> Result<Box<dyn Engine>, EngineError> {
        match spec {
            EngineSpec::Fake { script } => Ok(Box::new(FakeEngine::new(script.clone()))),
            EngineSpec::Whisper { .. } => Err(EngineError::Model),
        }
    }

    fn requests(reqs: &[Request]) -> Vec<u8> {
        let mut buf = Vec::new();
        for r in reqs {
            write_request(&mut buf, r).unwrap();
        }
        buf
    }

    fn replies(mut out: &[u8]) -> Vec<Reply> {
        let mut all = Vec::new();
        while let Some(r) = read_reply(&mut out).unwrap() {
            all.push(r);
        }
        all
    }

    fn init(script: Vec<FakeStep>) -> Request {
        Request::Init(Init {
            spec: EngineSpec::Fake { script },
            idle_secs: 60,
        })
    }

    fn phrase() -> Request {
        Request::Phrase(AudioBuf::from_vec(vec![0.1; 160]))
    }

    #[test]
    fn serve_answers_each_request_in_order() {
        let input = requests(&[
            init(vec![
                FakeStep::Text("Hello Alexandre.".into()),
                FakeStep::Fail,
            ]),
            Request::Start(Lang::English),
            phrase(),
            phrase(),
            Request::Stop,
        ]);
        let mut out = Vec::new();
        let end = serve(io::Cursor::new(input), &mut out, fake_maker);
        assert_eq!(end, ServeEnd::Closed);
        assert_eq!(
            replies(&out),
            vec![
                Reply::Ready,
                Reply::Started,
                Reply::Text("Hello Alexandre.".into()),
                Reply::Error(EngineError::Failed),
            ]
        );
    }

    #[test]
    fn serve_without_init_and_with_a_bad_model() {
        let input = requests(&[Request::Start(Lang::French), phrase()]);
        let mut out = Vec::new();
        serve(io::Cursor::new(input), &mut out, fake_maker);
        assert_eq!(
            replies(&out),
            vec![
                Reply::Error(EngineError::Model),
                Reply::Error(EngineError::Model)
            ]
        );
        let input = requests(&[Request::Init(Init {
            spec: EngineSpec::Whisper {
                model: PathBuf::from("missing.bin"),
                vad: None,
                threads: 1,
                gpu: false,
            },
            idle_secs: 60,
        })]);
        let mut out = Vec::new();
        serve(io::Cursor::new(input), &mut out, fake_maker);
        assert_eq!(replies(&out), vec![Reply::Error(EngineError::Model)]);
    }

    #[test]
    fn serve_ends_when_the_engine_is_gone() {
        let input = requests(&[
            init(vec![FakeStep::Exit]),
            Request::Start(Lang::English),
            phrase(),
            phrase(),
        ]);
        let mut out = Vec::new();
        let end = serve(io::Cursor::new(input), &mut out, fake_maker);
        assert_eq!(end, ServeEnd::EngineGone);
        assert_eq!(
            replies(&out),
            vec![
                Reply::Ready,
                Reply::Started,
                Reply::Error(EngineError::Exited)
            ]
        );
    }

    #[test]
    fn serve_breaks_on_a_bad_frame() {
        let mut input = requests(&[init(vec![])]);
        input.extend_from_slice(&[1, 0, 0, 0, 0x55]);
        let mut out = Vec::new();
        assert_eq!(
            serve(io::Cursor::new(input), &mut out, fake_maker),
            ServeEnd::Broken
        );
    }

    /// A reader that never ends and never yields: the pipe of an app that
    /// went quiet.
    struct Quiet;

    impl Read for Quiet {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            std::thread::sleep(Duration::from_secs(3600));
            Ok(0)
        }
    }

    #[test]
    fn serve_exits_when_idle() {
        // Init with a 1 s idle time, then silence.
        let init = requests(&[Request::Init(Init {
            spec: EngineSpec::Fake { script: vec![] },
            idle_secs: 1,
        })]);
        let input = io::Cursor::new(init).chain(Quiet);
        let mut out = Vec::new();
        let started = std::time::Instant::now();
        assert_eq!(serve(input, &mut out, fake_maker), ServeEnd::Idle);
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn the_worker_command_carries_no_data() {
        let command = worker_command(Path::new("Acta.exe"));
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, vec![std::ffi::OsStr::new(WORKER_ARG)]);
        assert_eq!(command.get_envs().count(), 0);
    }
}
