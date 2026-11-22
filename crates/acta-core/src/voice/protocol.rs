//! The worker's pipe protocol (ADR 0014, 5): length-prefixed frames over
//! the worker's stdin and stdout. Pipes are memory, not disk; nothing here
//! goes through a file or a process argument.
//!
//! A frame is a little-endian `u32` length, then that many bytes: a tag
//! byte and its payload. Requests (to the worker) and replies (from it):
//!
//! | Tag | Direction | Payload | Reply |
//! |-----|-----------|---------|-------|
//! | `Init` | to | JSON [`Init`] | `Ready` or `Error` |
//! | `Start` | to | language code | `Started` or `Error` |
//! | `Phrase` | to | 16 kHz mono `f32` LE samples | `Text` or `Error` |
//! | `Stop` | to | none | none |
//! | `Text` | from | UTF-8 | |
//! | `Error` | from | [`EngineError`] code | |
//!
//! Every buffer that held samples or text is zeroed once used.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::buffer::AudioBuf;
use super::engine::{EngineError, FakeStep, Lang};

/// The hidden argument that starts the GUI executable as the worker. The
/// only argument: it carries no data.
pub const WORKER_ARG: &str = "--voice-worker";

/// The largest frame: a 25 s phrase is 1.6 MB.
pub const MAX_FRAME: usize = 4 << 20;

/// How long an idle worker waits for a request before it exits, returning
/// the model's memory to the system.
pub const IDLE_EXIT: Duration = Duration::from_secs(5 * 60);

/// Which engine the worker runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum EngineSpec {
    Whisper {
        model: PathBuf,
        /// Silero VAD, from the app package; `None` when the package has
        /// none (development builds).
        vad: Option<PathBuf>,
        threads: u32,
        gpu: bool,
    },
    /// Scripted text for tests ([`super::engine::FakeEngine`]); the worker
    /// exits with [`EXIT_ENGINE_GONE`] at an `Exit` step, as a crash would.
    Fake { script: Vec<FakeStep> },
}

/// The first request: what to load and how long to idle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Init {
    pub spec: EngineSpec,
    pub idle_secs: u64,
}

/// The worker's exit code when its engine stops for good.
pub const EXIT_ENGINE_GONE: i32 = 3;

#[derive(Debug)]
pub enum Request {
    Init(Init),
    Start(Lang),
    Phrase(AudioBuf),
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Ready,
    Started,
    Text(String),
    Error(EngineError),
}

const INIT: u8 = 1;
const START: u8 = 2;
const PHRASE: u8 = 3;
const STOP: u8 = 4;
const READY: u8 = 0x81;
const STARTED: u8 = 0x82;
const TEXT: u8 = 0x83;
const ERROR: u8 = 0x84;

fn error_code(e: EngineError) -> u8 {
    match e {
        EngineError::Model => 1,
        EngineError::Exited => 2,
        EngineError::Failed => 3,
        EngineError::NotAllowed => 4,
    }
}

fn error_from(code: u8) -> Option<EngineError> {
    match code {
        1 => Some(EngineError::Model),
        2 => Some(EngineError::Exited),
        3 => Some(EngineError::Failed),
        4 => Some(EngineError::NotAllowed),
        _ => None,
    }
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

fn write_frame(w: &mut dyn Write, tag: u8, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len() + 1).map_err(|_| bad("frame too large"))?;
    if len as usize > MAX_FRAME {
        return Err(bad("frame too large"));
    }
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&[tag])?;
    w.write_all(payload)?;
    w.flush()
}

/// Reads one frame; `None` at a clean end of stream (between frames).
fn read_frame(r: &mut dyn Read) -> io::Result<Option<(u8, Zeroizing<Vec<u8>>)>> {
    let mut len = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut len[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_le_bytes(len) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(bad("frame length out of range"));
    }
    let mut body = Zeroizing::new(vec![0u8; len]);
    r.read_exact(&mut body)?;
    let tag = body[0];
    body.copy_within(1.., 0);
    let last = body.len() - 1;
    body[last] = 0;
    body.truncate(last);
    Ok(Some((tag, body)))
}

pub fn write_request(w: &mut dyn Write, req: &Request) -> io::Result<()> {
    match req {
        Request::Init(init) => {
            let json = serde_json::to_vec(init).map_err(|_| bad("init"))?;
            write_frame(w, INIT, &json)
        }
        Request::Start(lang) => write_frame(w, START, &[lang.code()]),
        Request::Phrase(samples) => write_phrase(w, samples.as_slice()),
        Request::Stop => write_frame(w, STOP, &[]),
    }
}

/// Writes a phrase from a borrowed slice (the engine trait's `feed`).
pub fn write_phrase(w: &mut dyn Write, samples: &[f32]) -> io::Result<()> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(samples.len() * 4));
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    write_frame(w, PHRASE, &bytes)
}

pub fn read_request(r: &mut dyn Read) -> io::Result<Option<Request>> {
    let Some((tag, body)) = read_frame(r)? else {
        return Ok(None);
    };
    let req = match tag {
        INIT => Request::Init(serde_json::from_slice(&body).map_err(|_| bad("init"))?),
        START => Request::Start(
            body.first()
                .and_then(|c| Lang::from_code(*c))
                .ok_or_else(|| bad("start"))?,
        ),
        PHRASE => {
            let (whole, rest) = body.as_chunks::<4>();
            if !rest.is_empty() {
                return Err(bad("phrase"));
            }
            let mut samples = AudioBuf::with_capacity(whole.len());
            for c in whole {
                samples.push(f32::from_le_bytes(*c));
            }
            Request::Phrase(samples)
        }
        STOP => Request::Stop,
        _ => return Err(bad("unknown request")),
    };
    Ok(Some(req))
}

pub fn write_reply(w: &mut dyn Write, reply: &Reply) -> io::Result<()> {
    match reply {
        Reply::Ready => write_frame(w, READY, &[]),
        Reply::Started => write_frame(w, STARTED, &[]),
        Reply::Text(t) => write_frame(w, TEXT, t.as_bytes()),
        Reply::Error(e) => write_frame(w, ERROR, &[error_code(*e)]),
    }
}

pub fn read_reply(r: &mut dyn Read) -> io::Result<Option<Reply>> {
    let Some((tag, body)) = read_frame(r)? else {
        return Ok(None);
    };
    let reply = match tag {
        READY => Reply::Ready,
        STARTED => Reply::Started,
        TEXT => Reply::Text(String::from_utf8(body.to_vec()).map_err(|e| {
            let mut bytes = e.into_bytes();
            bytes.zeroize();
            bad("text")
        })?),
        ERROR => Reply::Error(
            body.first()
                .and_then(|c| error_from(*c))
                .ok_or_else(|| bad("error"))?,
        ),
        _ => return Err(bad("unknown reply")),
    };
    Ok(Some(reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(req: Request) -> Request {
        let mut buf = Vec::new();
        write_request(&mut buf, &req).unwrap();
        let mut r = buf.as_slice();
        let back = read_request(&mut r).unwrap().unwrap();
        assert!(read_request(&mut r).unwrap().is_none());
        back
    }

    #[test]
    fn requests_round_trip() {
        let init = Init {
            spec: EngineSpec::Whisper {
                model: PathBuf::from("models/ggml-small-q5_1.bin"),
                vad: None,
                threads: 3,
                gpu: false,
            },
            idle_secs: 300,
        };
        match round_trip(Request::Init(init.clone())) {
            Request::Init(back) => assert_eq!(back, init),
            other => panic!("{other:?}"),
        }
        let fake = Init {
            spec: EngineSpec::Fake {
                script: vec![
                    FakeStep::Text("Bonjour Alexandre.".into()),
                    FakeStep::Stall(Duration::from_millis(5), "x".into()),
                    FakeStep::Fail,
                    FakeStep::Exit,
                ],
            },
            idle_secs: 1,
        };
        match round_trip(Request::Init(fake.clone())) {
            Request::Init(back) => assert_eq!(back, fake),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            round_trip(Request::Start(Lang::French)),
            Request::Start(Lang::French)
        ));
        let samples = [0.0, -1.0, 0.5, f32::MIN_POSITIVE];
        match round_trip(Request::Phrase(AudioBuf::from_vec(samples.to_vec()))) {
            Request::Phrase(back) => assert_eq!(back.as_slice(), &samples),
            other => panic!("{other:?}"),
        }
        assert!(matches!(round_trip(Request::Stop), Request::Stop));
    }

    #[test]
    fn replies_round_trip() {
        for reply in [
            Reply::Ready,
            Reply::Started,
            Reply::Text("Écrit par Alexandre.".into()),
            Reply::Text(String::new()),
            Reply::Error(EngineError::Model),
            Reply::Error(EngineError::Exited),
            Reply::Error(EngineError::Failed),
            Reply::Error(EngineError::NotAllowed),
        ] {
            let mut buf = Vec::new();
            write_reply(&mut buf, &reply).unwrap();
            let back = read_reply(&mut buf.as_slice()).unwrap().unwrap();
            assert_eq!(back, reply);
        }
    }

    #[test]
    fn bad_frames_are_errors_not_panics() {
        // Truncated length, zero length, oversize, unknown tag, truncated body.
        let cases: [&[u8]; 5] = [
            &[1, 0],
            &[0, 0, 0, 0],
            &[0, 0, 0, 0x7f],
            &[1, 0, 0, 0, 0x55],
            &[9, 0, 0, 0, PHRASE, 0, 0],
        ];
        for case in cases {
            assert!(read_request(&mut &case[..]).is_err(), "{case:?}");
            assert!(read_reply(&mut &case[..]).is_err(), "{case:?}");
        }
        // A phrase whose length is not a whole number of samples.
        assert!(read_request(&mut &[4u8, 0, 0, 0, PHRASE, 1, 2, 3][..]).is_err());
        // Invalid UTF-8 text.
        assert!(read_reply(&mut &[3u8, 0, 0, 0, TEXT, 0xff, 0xfe][..]).is_err());
    }
}
