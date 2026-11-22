//! Dictation (ADR 0014): everything that is not audio capture, the UI or
//! whisper.cpp itself. Engine-independent: the phrase segmenter, the
//! anchor and insertion rules, the take state machine, the input level,
//! and the model table and recommendation.
//!
//! Audio and the text it becomes are user content (ADR 0014, 7): nothing
//! here writes either to disk or to a log, and every audio buffer is an
//! [`buffer::AudioBuf`], zeroed when its phrase is done.

pub mod buffer;
pub mod engine;
pub mod hardware;
pub mod insert;
pub mod level;
pub mod models;
pub mod segment;
pub mod take;

/// The one sample rate of everything here: 16 kHz mono `f32`, Whisper's input.
pub const SAMPLE_RATE: usize = 16_000;

/// Samples in `ms` milliseconds at [`SAMPLE_RATE`].
pub const fn samples(ms: usize) -> usize {
    SAMPLE_RATE / 1000 * ms
}
