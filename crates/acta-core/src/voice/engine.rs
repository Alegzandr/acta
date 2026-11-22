//! The engine trait (ADR 0014, 5): 16 kHz mono `f32` phrases in, text out.
//! Implementations: the Whisper worker process (through
//! [`super::worker::WorkerClient`]), the macOS recogniser (in the app,
//! macOS only), and
//! [`FakeEngine`] for tests. Everything above the trait (segmentation,
//! anchors, insertion, limits, states) is engine-independent.

use std::fmt;
use std::time::Duration;

/// The take's language setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// "English or French, detected": detected on the first phrase of a
    /// take and kept for the take.
    Detect,
    English,
    French,
}

impl Lang {
    pub fn code(self) -> u8 {
        match self {
            Lang::Detect => 0,
            Lang::English => 1,
            Lang::French => 2,
        }
    }

    pub fn from_code(code: u8) -> Option<Lang> {
        match code {
            0 => Some(Lang::Detect),
            1 => Some(Lang::English),
            2 => Some(Lang::French),
            _ => None,
        }
    }
}

/// Why an engine could not give text. Carries no text and no audio, so it
/// can be logged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// The model file is missing or fails its check (`voice.error.model`).
    Model,
    /// The engine stopped: the worker process exited or broke the protocol
    /// (`voice.error.engine`).
    Exited,
    /// The engine is running but this phrase failed.
    Failed,
    /// The system refused the engine (the macOS recogniser's speech
    /// recognition permission) (`voice.error.speech_blocked`).
    NotAllowed,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EngineError::Model => "the model is missing or damaged",
            EngineError::Exited => "the engine stopped",
            EngineError::Failed => "the engine failed on a phrase",
            EngineError::NotAllowed => "the system refused the engine",
        })
    }
}

impl std::error::Error for EngineError {}

pub trait Engine: Send {
    /// A take starts. May return before the model is loaded; the first
    /// `feed` waits for it.
    fn start(&mut self, lang: Lang) -> Result<(), EngineError>;
    /// The text of one phrase; empty when it held no speech.
    fn feed(&mut self, phrase: &[f32]) -> Result<String, EngineError>;
    /// The take ends; context from it is dropped.
    fn stop(&mut self);
    /// Lets go of the model: the worker process exits, so its file can be
    /// replaced or deleted. The next `start` loads it again.
    fn release(&mut self) {}
}

/// What [`FakeEngine`] does for one phrase.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FakeStep {
    Text(String),
    /// Fails this phrase ([`EngineError::Failed`]).
    Fail,
    /// Waits, then gives the text.
    Stall(Duration, String),
    /// Stops for good ([`EngineError::Exited`] from now on; the worker in
    /// test mode exits the process instead).
    Exit,
}

/// A scripted engine: one step per phrase, in order; past the script,
/// every phrase gives "phrase N".
#[derive(Debug, Default)]
pub struct FakeEngine {
    script: Vec<FakeStep>,
    next: usize,
    exited: bool,
    started: Option<Lang>,
    /// Phrase lengths fed, for assertions (never the samples).
    pub fed: Vec<usize>,
    pub fail_start: bool,
    /// How many times `release` was called.
    pub released: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeEngine {
    pub fn new(script: Vec<FakeStep>) -> Self {
        FakeEngine {
            script,
            ..Self::default()
        }
    }

    pub fn started(&self) -> Option<Lang> {
        self.started
    }
}

impl Engine for FakeEngine {
    fn start(&mut self, lang: Lang) -> Result<(), EngineError> {
        if self.exited {
            return Err(EngineError::Exited);
        }
        if self.fail_start {
            return Err(EngineError::Model);
        }
        self.started = Some(lang);
        Ok(())
    }

    fn feed(&mut self, phrase: &[f32]) -> Result<String, EngineError> {
        if self.exited {
            return Err(EngineError::Exited);
        }
        self.fed.push(phrase.len());
        let n = self.next;
        self.next += 1;
        match self.script.get(n).cloned() {
            None => Ok(format!("phrase {}", n + 1)),
            Some(FakeStep::Text(t)) => Ok(t),
            Some(FakeStep::Fail) => Err(EngineError::Failed),
            Some(FakeStep::Stall(d, t)) => {
                std::thread::sleep(d);
                Ok(t)
            }
            Some(FakeStep::Exit) => {
                self.exited = true;
                Err(EngineError::Exited)
            }
        }
    }

    fn stop(&mut self) {
        self.started = None;
    }

    fn release(&mut self) {
        self.started = None;
        self.released
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_follows_its_script() {
        let mut e = FakeEngine::new(vec![
            FakeStep::Text("Bonjour Alexandre.".into()),
            FakeStep::Fail,
            FakeStep::Stall(Duration::from_millis(1), "Late.".into()),
            FakeStep::Exit,
        ]);
        e.start(Lang::French).unwrap();
        assert_eq!(e.started(), Some(Lang::French));
        assert_eq!(e.feed(&[0.0; 10]).unwrap(), "Bonjour Alexandre.");
        assert_eq!(e.feed(&[0.0; 10]), Err(EngineError::Failed));
        assert_eq!(e.feed(&[0.0; 10]).unwrap(), "Late.");
        assert_eq!(e.feed(&[0.0; 10]), Err(EngineError::Exited));
        assert_eq!(e.feed(&[0.0; 10]), Err(EngineError::Exited));
        assert_eq!(e.start(Lang::English), Err(EngineError::Exited));
        assert_eq!(e.fed, vec![10; 4]);
    }

    #[test]
    fn past_the_script_phrases_are_numbered() {
        let mut e = FakeEngine::new(vec![]);
        e.start(Lang::Detect).unwrap();
        assert_eq!(e.feed(&[]).unwrap(), "phrase 1");
        assert_eq!(e.feed(&[]).unwrap(), "phrase 2");
    }

    #[test]
    fn lang_codes_round_trip() {
        for l in [Lang::Detect, Lang::English, Lang::French] {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
        assert_eq!(Lang::from_code(9), None);
    }
}
