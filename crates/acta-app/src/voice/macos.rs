//! The macOS recogniser (ADR 0014, 3): `SFSpeechRecognizer` with
//! `requiresOnDeviceRecognition` always set, so audio never leaves the Mac.
//! Offered in Settings > Voice only where `supportsOnDeviceRecognition` is
//! true for the language ([`on_device`]), and only in a bundle whose
//! `Info.plist` declares `NSSpeechRecognitionUsageDescription` (without it
//! the permission request terminates the process: a `cargo run` binary
//! never offers it).
//!
//! Each phrase is one recognition request on a recogniser made for it, on
//! the controller's engine thread; the framework answers on a queue of its
//! own and the thread waits for the final result. Nothing here holds an
//! Objective-C object between calls, so the engine is `Send` by
//! construction. Neither audio nor text is logged.

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

use acta_core::voice::engine::{Engine, EngineError, Lang};
use block2::RcBlock;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::NSObjectProtocol;
use objc2::{AllocAnyThread, sel};
use objc2_avf_audio::{AVAudioCommonFormat, AVAudioFormat, AVAudioPCMBuffer};
use objc2_foundation::{NSBundle, NSError, NSLocale, NSOperationQueue, NSString, ns_string};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognizer,
    SFSpeechRecognizerAuthorizationStatus,
};

use super::OnDevice;

/// Longest wait for one phrase's final result (a phrase is at most 25 s).
const PHRASE_LIMIT: Duration = Duration::from_secs(30);
/// Longest wait for the person to answer the permission prompt.
const PROMPT_LIMIT: Duration = Duration::from_secs(300);
/// `kAFAssistantErrorDomain` 1110: "No speech detected". Not an error for
/// dictation: the phrase held no words.
/// Any other domain with that code is a real failure.
const NO_SPEECH_DOMAIN: &str = "kAFAssistantErrorDomain";
const NO_SPEECH: isize = 1110;

/// Which of Acta's languages the recogniser takes on this Mac without the
/// network. Blocking for a few milliseconds: call it off the UI thread.
pub fn on_device() -> OnDevice {
    if !usage_declared() {
        return OnDevice::default();
    }
    autoreleasepool(|_| OnDevice {
        en: recognizer("en").is_some(),
        fr: recognizer("fr").is_some(),
    })
}

/// The bundle declares why it asks: without the key the system ends the
/// process at the permission request.
fn usage_declared() -> bool {
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(ns_string!("NSSpeechRecognitionUsageDescription"))
        .is_some()
}

/// The person's own locale when it is in `language` (en-GB, fr-CA), else
/// the language's main one.
fn locale_for(language: &str) -> Retained<NSLocale> {
    let current = NSLocale::currentLocale();
    if current.languageCode().to_string() == language {
        return current;
    }
    let id = if language == "fr" { "fr-FR" } else { "en-US" };
    NSLocale::initWithLocaleIdentifier(NSLocale::alloc(), &NSString::from_str(id))
}

/// A recogniser for `language` that works on the device, or `None`. The
/// framework falls back to the keyboard's dictation language for a locale
/// it does not support: that fallback is refused.
fn recognizer(language: &str) -> Option<Retained<SFSpeechRecognizer>> {
    let locale = locale_for(language);
    // SAFETY: plain Objective-C calls on objects created here.
    unsafe {
        let r = SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)?;
        let same = r.locale().languageCode().to_string() == language;
        (same && r.supportsOnDeviceRecognition()).then_some(r)
    }
}

/// The permission, asked the first time (the system's prompt) and read
/// after that. Blocks the engine thread while the prompt is up.
fn authorize() -> Result<(), EngineError> {
    if !usage_declared() {
        return Err(EngineError::NotAllowed);
    }
    // SAFETY: class methods without arguments; the block only sends.
    let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
    let status = if status == SFSpeechRecognizerAuthorizationStatus::NotDetermined {
        let (tx, rx) = mpsc::channel();
        let handler = RcBlock::new(move |s: SFSpeechRecognizerAuthorizationStatus| {
            let _ = tx.send(s);
        });
        unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };
        rx.recv_timeout(PROMPT_LIMIT)
            .unwrap_or(SFSpeechRecognizerAuthorizationStatus::NotDetermined)
    } else {
        status
    };
    if status == SFSpeechRecognizerAuthorizationStatus::Authorized {
        Ok(())
    } else {
        Err(EngineError::NotAllowed)
    }
}

/// The engine. `fallback` is the language "English or French, detected"
/// uses: the recogniser has no detection, so it is the interface's.
pub struct MacRecognizer {
    fallback: &'static str,
    language: Option<&'static str>,
}

impl MacRecognizer {
    pub fn new(french_interface: bool) -> Self {
        MacRecognizer {
            fallback: if french_interface { "fr" } else { "en" },
            language: None,
        }
    }
}

impl Engine for MacRecognizer {
    fn start(&mut self, lang: Lang) -> Result<(), EngineError> {
        authorize()?;
        let language = match lang {
            Lang::English => "en",
            Lang::French => "fr",
            Lang::Detect => self.fallback,
        };
        // Settings offers this engine only where it works on the device;
        // a language changed since then ends here.
        autoreleasepool(|_| recognizer(language).map(|_| ())).ok_or(EngineError::Exited)?;
        self.language = Some(language);
        Ok(())
    }

    fn feed(&mut self, phrase: &[f32]) -> Result<String, EngineError> {
        let language = self.language.ok_or(EngineError::Exited)?;
        if phrase.is_empty() {
            return Ok(String::new());
        }
        autoreleasepool(|_| recognise(language, phrase))
    }

    fn stop(&mut self) {
        self.language = None;
    }
}

/// What the result handler sends back.
enum Answer {
    Text(String),
    NoSpeech,
    Failed,
}

fn recognise(language: &str, phrase: &[f32]) -> Result<String, EngineError> {
    let r = recognizer(language).ok_or(EngineError::Exited)?;
    let frames = u32::try_from(phrase.len()).map_err(|_| EngineError::Failed)?;
    // SAFETY: Objective-C calls on objects created here. The buffer is
    // made for `frames` mono float samples, non-interleaved, so channel 0
    // holds `frames` floats; it is filled once and zeroed before release.
    unsafe {
        if !r.isAvailable() {
            return Err(EngineError::Failed);
        }
        // Answers on a queue of its own, never the main thread's.
        let queue = NSOperationQueue::new();
        r.setQueue(&queue);

        let format = AVAudioFormat::initWithCommonFormat_sampleRate_channels_interleaved(
            AVAudioFormat::alloc(),
            AVAudioCommonFormat::PCMFormatFloat32,
            16_000.0,
            1,
            false,
        )
        .ok_or(EngineError::Failed)?;
        let buffer = AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            frames,
        )
        .ok_or(EngineError::Failed)?;
        let channels = buffer.floatChannelData();
        let Some(channel) = NonNull::new(channels).map(|c| c.read().as_ptr()) else {
            return Err(EngineError::Failed);
        };
        std::ptr::copy_nonoverlapping(phrase.as_ptr(), channel, phrase.len());
        buffer.setFrameLength(frames);

        let request = SFSpeechAudioBufferRecognitionRequest::new();
        // ADR 0014, 2: on this Mac or not at all.
        request.setRequiresOnDeviceRecognition(true);
        request.setShouldReportPartialResults(false);
        // macOS 13 and later; older systems give unpunctuated text.
        if request.respondsToSelector(sel!(setAddsPunctuation:)) {
            request.setAddsPunctuation(true);
        }
        request.appendAudioPCMBuffer(&buffer);
        request.endAudio();

        let (tx, rx) = mpsc::channel();
        let handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                let answer = if let Some(error) = error.as_ref() {
                    if error.code() == NO_SPEECH && error.domain().to_string() == NO_SPEECH_DOMAIN {
                        Answer::NoSpeech
                    } else {
                        Answer::Failed
                    }
                } else if let Some(result) = result.as_ref() {
                    if !result.isFinal() {
                        return;
                    }
                    Answer::Text(result.bestTranscription().formattedString().to_string())
                } else {
                    Answer::Failed
                };
                let _ = tx.send(answer);
            },
        );
        let task = r.recognitionTaskWithRequest_resultHandler(&request, &handler);
        let answer = rx.recv_timeout(PHRASE_LIMIT);
        if answer.is_err() {
            task.cancel();
        }
        // The framework keeps its own copy; Acta's is wiped (ADR 0014, 7).
        std::ptr::write_bytes(channel, 0, phrase.len());
        match answer {
            Ok(Answer::Text(text)) => Ok(text),
            Ok(Answer::NoSpeech) => Ok(String::new()),
            Ok(Answer::Failed) | Err(_) => Err(EngineError::Failed),
        }
    }
}
