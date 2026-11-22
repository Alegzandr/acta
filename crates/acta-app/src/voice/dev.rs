//! Debug builds only: dictation driven without a microphone or a model,
//! for captures and checks on a machine someone is using.
//!
//! `ACTA_DEV_VOICE` picks one:
//! - `fake`: real takes through the whole pipeline (capture thread,
//!   resampler, segmenter, take controller), fed by generated tone bursts
//!   instead of a device, with an in-process scripted engine that returns
//!   the phrases of `ACTA_DEV_VOICE_TEXT` (`|`-separated);
//! - a look for every editor: `starting`, `listening`, `writing`, `setup`,
//!   `downloading`, or a line: `blocked`, `no_device`, `device_lost`,
//!   `model`, `engine`, `nothing`, `silence`, `limit`;
//! - the first-use dialog in a state: `dialog-offer`, `dialog-no-space`,
//!   `dialog-unavailable`, `dialog-unsupported`, `dialog-downloading`,
//!   `dialog-checking`, `dialog-ready`, `dialog-failed`;
//! - `settings`: Settings > Voice with a model in use and one downloading.
//!
//! `ACTA_DEV_VOICE_URL` points the downloader at a local test server
//! (plain HTTP to the loopback address is then allowed).

use std::time::{Duration, Instant};

use acta_core::voice::buffer::AudioBuf;
use acta_core::voice::download::DownloadError;
use acta_core::voice::engine::{EngineError, FakeEngine, FakeStep};
use acta_core::voice::models::ModelId;
use acta_core::voice::take::Notice;

use super::capture::CaptureError;
use super::view::{Line, Phase, RowState, Setup};

/// Each model's forced row state.
pub type RowsOf = fn(ModelId) -> RowState;

#[derive(Default)]
pub struct Dev {
    which: Option<String>,
    /// `fake`: the scripted phrases.
    pub fake: Option<Vec<String>>,
    pub url: Option<String>,
    pub line: Option<Line>,
    pub level: Option<f32>,
}

const SAMPLE: &str = "Talked to Alexandre about the export format.|He wants the passphrase option before 1.0, so we keep format 2 open until then.";

impl Dev {
    pub fn from_env() -> Self {
        let which = std::env::var("ACTA_DEV_VOICE")
            .ok()
            .filter(|v| !v.is_empty());
        let fake = which
            .as_deref()
            .is_some_and(|w| w.starts_with("fake"))
            .then(|| {
                std::env::var("ACTA_DEV_VOICE_TEXT")
                    .unwrap_or_else(|_| SAMPLE.into())
                    .split('|')
                    .map(str::to_owned)
                    .collect()
            });
        let line = match which.as_deref() {
            Some("blocked") => Some(Line::Capture(CaptureError::Blocked)),
            Some("no_device") => Some(Line::Capture(CaptureError::NoDevice)),
            Some("device_lost") => Some(Line::Capture(CaptureError::DeviceLost)),
            Some("model") => Some(Line::Engine(EngineError::Model)),
            Some("engine") => Some(Line::Engine(EngineError::Exited)),
            Some("nothing") => Some(Line::Notice(Notice::NothingHeard)),
            Some("silence") => Some(Line::Notice(Notice::StoppedSilence)),
            Some("limit") => Some(Line::Notice(Notice::StoppedLimit)),
            _ => None,
        };
        let level = match which.as_deref() {
            Some("listening" | "settings") => Some(-18.0),
            _ => None,
        };
        Dev {
            url: std::env::var("ACTA_DEV_VOICE_URL")
                .ok()
                .filter(|v| !v.is_empty()),
            which,
            fake,
            line,
            level,
        }
    }

    /// Any hook is on: no device is opened or listed.
    pub fn active(&self) -> bool {
        self.which.is_some()
    }

    pub fn which(&self) -> Option<&str> {
        self.which.as_deref()
    }

    /// The look every editor's button takes, if forced.
    pub fn phase(&self, now: Instant) -> Option<Phase> {
        let past = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
        Some(match self.which.as_deref()? {
            "starting" => Phase::Starting { since: past },
            "listening" => Phase::Listening {
                level: -18.0,
                elapsed: Duration::from_secs(12),
            },
            "writing" => Phase::Writing { since: past },
            "setup" => Phase::NotSetUp,
            "downloading" => Phase::Downloading(45),
            _ if self.line.is_some() => Phase::Idle,
            _ => return None,
        })
    }

    /// The first-use dialog's state, if forced.
    pub fn setup(&self) -> Option<Setup> {
        const GB: u64 = 1 << 30;
        let small = ModelId::Small;
        Some(match self.which.as_deref()? {
            "dialog-offer" => Setup::Offer {
                model: small,
                gpu: false,
                free: Some(112 * GB),
                enough: true,
            },
            "dialog-no-space" => Setup::Offer {
                model: ModelId::LargeTurbo,
                gpu: true,
                free: Some(310 << 20),
                enough: false,
            },
            "dialog-unavailable" => Setup::TooLittleMemory { memory: 3 * GB / 2 },
            "dialog-unsupported" => Setup::UnsupportedCpu,
            "dialog-downloading" => Setup::Downloading {
                model: small,
                done: 120 << 20,
                total: small.file().size,
            },
            "dialog-checking" => Setup::Checking { model: small },
            "dialog-ready" => Setup::Ready,
            "dialog-failed" => Setup::Failed {
                model: small,
                error: DownloadError::Mismatch,
            },
            _ => return None,
        })
    }

    /// Settings > Voice with a model in use and one downloading.
    pub fn rows(&self) -> Option<(RowsOf, u64)> {
        (self.which.as_deref() == Some("settings")).then_some((
            |m| match m {
                ModelId::Small => RowState::InUse,
                ModelId::LargeTurbo => RowState::Downloading(45),
                _ => RowState::NotDownloaded,
            },
            112 << 30,
        ))
    }

    pub fn fake_engine(&self) -> FakeEngine {
        let script = self
            .fake
            .iter()
            .flatten()
            .map(|t| FakeStep::Text(t.clone()))
            .collect();
        FakeEngine::new(script)
    }
}

/// The generated input of a `fake` take: for each scripted phrase, 1.2 s
/// of a tone and 0.9 s of silence, as 48 kHz stereo.
pub fn signal() -> (u32, u16, AudioBuf) {
    let rate = 48_000u32;
    let phrases = Dev::from_env().fake.map_or(1, |f| f.len().max(1));
    let mut out = AudioBuf::new();
    let mut push = |seconds: f32, amplitude: f32| {
        let frames = (rate as f32 * seconds) as usize;
        for i in 0..frames {
            let s = amplitude * (i as f32 * 220.0 * std::f32::consts::TAU / rate as f32).sin();
            out.push(s);
            out.push(s);
        }
    };
    push(0.4, 0.0);
    for _ in 0..phrases {
        push(1.2, 0.3);
        push(0.9, 0.0);
    }
    (rate, 2, out)
}
