//! The phrase segmenter (ADR 0014, 5 and 6): 16 kHz mono samples in,
//! phrases out. An energy detector over 10 ms frames against an adaptive
//! noise floor; a phrase ends after 600 ms under the threshold, or is cut
//! at 25 s (under Whisper's 30 s window). A take stops by itself after
//! 60 s without speech and after 10 minutes in all.
//!
//! Engine-independent and deterministic: the same samples give the same
//! phrases, whatever the chunk sizes they arrive in.

use super::buffer::AudioBuf;
use super::samples;

/// One analysis frame: 10 ms.
const FRAME: usize = samples(10);
/// The pause that ends a phrase.
pub const PAUSE: usize = samples(600);
/// The longest phrase.
pub const MAX_PHRASE: usize = samples(25_000);
/// Silence that stops a take.
pub const SILENCE_STOP: usize = samples(60_000);
/// The longest take.
pub const TAKE_LIMIT: usize = samples(600_000);
/// Audio kept before the first loud frame, so a soft onset is not clipped.
const PRE_ROLL: usize = samples(200);
/// Audio kept after the last loud frame.
const TAIL: usize = samples(200);
/// A phrase with less loud audio than this is a click, not speech.
const MIN_SPEECH: usize = samples(100);

/// Speech is this far above the noise floor.
const MARGIN_DB: f32 = 9.0;
/// ...and never quieter than this, whatever the floor.
const ABSOLUTE_MIN_DB: f32 = -55.0;
/// The floor follows a quieter frame at once and rises at this rate, so a
/// fan turned on is learnt in seconds while 25 s of speech is not mistaken
/// for noise.
const FLOOR_RISE_DB_PER_FRAME: f32 = 1.0 / 100.0;
const SILENT_DB: f32 = -100.0;
/// The first frames only learn the floor (their minimum); they go to the
/// pre-roll, so words spoken at once are not lost.
const CALIBRATION_FRAMES: usize = PRE_ROLL / FRAME;

/// What the segmenter tells the take.
#[derive(Debug)]
pub enum Event {
    /// A phrase to decode, in order. Its buffer is wiped when dropped.
    Phrase(AudioBuf),
    /// 60 s without speech: the take stops (`voice.stopped_silence`).
    SilenceStop,
    /// 10 minutes: the take stops (`voice.stopped_limit`). Any phrase in
    /// progress was emitted just before.
    LimitStop,
}

pub struct Segmenter {
    floor_db: f32,
    frames: usize,
    /// The current partial frame.
    frame: AudioBuf,
    /// The last [`PRE_ROLL`] samples while no phrase is open.
    pre_roll: AudioBuf,
    /// The open phrase, if any.
    phrase: AudioBuf,
    in_phrase: bool,
    /// Loud samples in the open phrase.
    speech: usize,
    /// Quiet samples since the last loud frame of the open phrase.
    quiet_run: usize,
    /// Samples since the last loud frame of the take (or its start).
    since_speech: usize,
    total: usize,
    stopped: bool,
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Segmenter {
    pub fn new() -> Self {
        Segmenter {
            floor_db: 0.0,
            frames: 0,
            frame: AudioBuf::with_capacity(FRAME),
            pre_roll: AudioBuf::with_capacity(PRE_ROLL + FRAME),
            phrase: AudioBuf::new(),
            in_phrase: false,
            speech: 0,
            quiet_run: 0,
            since_speech: 0,
            total: 0,
            stopped: false,
        }
    }

    /// Feeds samples; events come out in order. After a stop event the
    /// segmenter ignores further input.
    pub fn push(&mut self, mut samples: &[f32], events: &mut Vec<Event>) {
        while !samples.is_empty() && !self.stopped {
            let take = (FRAME - self.frame.len()).min(samples.len());
            self.frame.extend_from_slice(&samples[..take]);
            samples = &samples[take..];
            if self.frame.len() == FRAME {
                self.frame_done(events);
            }
        }
    }

    /// The take ends (stop key, focus lost): the open phrase, if it holds
    /// speech, comes out; every buffer is wiped.
    pub fn finish(&mut self, events: &mut Vec<Event>) {
        if !self.stopped && !self.frame.is_empty() && self.in_phrase {
            let frame = self.frame.take();
            self.phrase.extend_from_slice(frame.as_slice());
        }
        self.close_phrase(events);
        self.wipe();
    }

    /// Drops everything not yet emitted (`Esc`), wiping it.
    pub fn discard(&mut self) {
        self.wipe();
    }

    fn wipe(&mut self) {
        self.frame.wipe();
        self.pre_roll.wipe();
        self.phrase.wipe();
        self.in_phrase = false;
        self.speech = 0;
        self.quiet_run = 0;
        self.stopped = true;
    }

    fn frame_done(&mut self, events: &mut Vec<Event>) {
        let db = frame_db(self.frame.as_slice());
        let calibrating = self.frames < CALIBRATION_FRAMES;
        self.frames += 1;
        let loud = !calibrating && db > (self.floor_db + MARGIN_DB).max(ABSOLUTE_MIN_DB);
        self.floor_db = if db < self.floor_db {
            db
        } else if calibrating {
            self.floor_db
        } else {
            self.floor_db + FLOOR_RISE_DB_PER_FRAME
        };
        let frame = self.frame.take();
        self.total += FRAME;

        if loud {
            self.since_speech = 0;
            if !self.in_phrase {
                self.in_phrase = true;
                let pre = self.pre_roll.take();
                self.phrase.extend_from_slice(pre.as_slice());
            }
            self.phrase.extend_from_slice(frame.as_slice());
            self.speech += FRAME;
            self.quiet_run = 0;
        } else {
            self.since_speech += FRAME;
            if self.in_phrase {
                self.quiet_run += FRAME;
                if self.quiet_run <= TAIL {
                    self.phrase.extend_from_slice(frame.as_slice());
                }
                if self.quiet_run >= PAUSE {
                    self.close_phrase(events);
                }
            } else {
                self.pre_roll.extend_from_slice(frame.as_slice());
                let over = self.pre_roll.len().saturating_sub(PRE_ROLL);
                self.pre_roll.drain_front(over);
            }
        }

        if self.in_phrase && self.phrase.len() >= MAX_PHRASE {
            self.close_phrase(events);
        }
        if self.total >= TAKE_LIMIT {
            self.close_phrase(events);
            events.push(Event::LimitStop);
            self.wipe();
        } else if self.since_speech >= SILENCE_STOP {
            self.close_phrase(events);
            events.push(Event::SilenceStop);
            self.wipe();
        }
    }

    fn close_phrase(&mut self, events: &mut Vec<Event>) {
        if self.in_phrase && self.speech >= MIN_SPEECH {
            events.push(Event::Phrase(self.phrase.take()));
        } else {
            self.phrase.wipe();
        }
        self.in_phrase = false;
        self.speech = 0;
        self.quiet_run = 0;
    }
}

fn frame_db(frame: &[f32]) -> f32 {
    let ms = frame.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / frame.len() as f64;
    if ms <= 1e-10 {
        SILENT_DB
    } else {
        (10.0 * ms.log10()) as f32
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::voice::SAMPLE_RATE;

    /// Generated audio: tone bursts, silence and white noise at set levels.
    /// Never a recording, never a file.
    pub(crate) struct Gen {
        pub samples: Vec<f32>,
        seed: u32,
    }

    impl Gen {
        pub fn new() -> Self {
            Gen {
                samples: Vec::new(),
                seed: 0x1234_5678,
            }
        }

        fn noise_sample(&mut self) -> f32 {
            // xorshift32: deterministic white noise in [-1, 1).
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0
        }

        /// A 300 Hz tone at `db` dBFS RMS, with `noise_db` noise under it.
        pub fn tone(mut self, ms: usize, db: f32, noise_db: Option<f32>) -> Self {
            let amp = 10f32.powf(db / 20.0) * std::f32::consts::SQRT_2;
            for i in 0..samples(ms) {
                let t = i as f32 / SAMPLE_RATE as f32;
                let mut s = amp * (t * 300.0 * std::f32::consts::TAU).sin();
                if let Some(n) = noise_db {
                    s += self.noise_at(n);
                }
                self.samples.push(s);
            }
            self
        }

        fn noise_at(&mut self, db: f32) -> f32 {
            // Uniform noise in [-1, 1) has an RMS of 1/sqrt(3).
            let amp = 10f32.powf(db / 20.0) * 3f32.sqrt();
            self.noise_sample() * amp
        }

        pub fn silence(mut self, ms: usize) -> Self {
            self.samples.extend(std::iter::repeat_n(0.0, samples(ms)));
            self
        }

        pub fn noise(mut self, ms: usize, db: f32) -> Self {
            for _ in 0..samples(ms) {
                let s = self.noise_at(db);
                self.samples.push(s);
            }
            self
        }
    }

    /// Runs the whole buffer through in `chunk`-sized pieces, then finishes.
    fn run(audio: &[f32], chunk: usize) -> Vec<Event> {
        let mut seg = Segmenter::new();
        let mut events = Vec::new();
        for piece in audio.chunks(chunk) {
            seg.push(piece, &mut events);
        }
        seg.finish(&mut events);
        events
    }

    fn phrase_lengths(events: &[Event]) -> Vec<usize> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Phrase(p) => Some(p.len()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn two_bursts_800_ms_apart_are_two_phrases() {
        let audio = Gen::new()
            .silence(500)
            .tone(1000, -20.0, None)
            .silence(800)
            .tone(1000, -20.0, None)
            .silence(1000);
        let events = run(&audio.samples, 480);
        assert_eq!(phrase_lengths(&events).len(), 2, "{events:?}");
        // Each phrase: the pre-roll, the burst and the tail.
        for len in phrase_lengths(&events) {
            assert_eq!(len, samples(200 + 1000 + 200));
        }
    }

    #[test]
    fn two_bursts_400_ms_apart_are_one_phrase() {
        let audio = Gen::new()
            .silence(500)
            .tone(1000, -20.0, None)
            .silence(400)
            .tone(1000, -20.0, None)
            .silence(1000);
        let events = run(&audio.samples, 1);
        assert_eq!(phrase_lengths(&events).len(), 1, "{events:?}");
    }

    #[test]
    fn the_same_audio_gives_the_same_phrases_whatever_the_chunks() {
        let audio = Gen::new()
            .noise(700, -50.0)
            .tone(900, -25.0, Some(-50.0))
            .noise(700, -50.0)
            .tone(300, -25.0, Some(-50.0))
            .noise(200, -50.0);
        let a = phrase_lengths(&run(&audio.samples, 1));
        let b = phrase_lengths(&run(&audio.samples, 4096));
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn speech_over_noise_is_found_and_noise_alone_is_not() {
        let audio = Gen::new()
            .noise(2000, -40.0)
            .tone(1000, -20.0, Some(-40.0))
            .noise(2000, -40.0);
        assert_eq!(phrase_lengths(&run(&audio.samples, 160)).len(), 1);
        let noise = Gen::new().noise(10_000, -40.0);
        assert!(phrase_lengths(&run(&noise.samples, 160)).is_empty());
    }

    #[test]
    fn thirty_seconds_of_speech_is_cut_at_25() {
        let audio = Gen::new()
            .silence(300)
            .tone(30_000, -20.0, None)
            .silence(1000);
        let lengths = phrase_lengths(&run(&audio.samples, 1600));
        assert_eq!(lengths.len(), 2, "{lengths:?}");
        assert_eq!(lengths[0], MAX_PHRASE);
        assert!(lengths[1] > samples(4_500) && lengths[1] < samples(5_500));
    }

    #[test]
    fn silence_alone_gives_nothing_and_stops_at_60_s() {
        let audio = Gen::new().silence(61_000);
        let mut seg = Segmenter::new();
        let mut events = Vec::new();
        let mut stopped_at = None;
        for (i, piece) in audio.samples.chunks(160).enumerate() {
            seg.push(piece, &mut events);
            if stopped_at.is_none() && !events.is_empty() {
                stopped_at = Some((i + 1) * 160);
            }
        }
        assert!(
            matches!(events.as_slice(), [Event::SilenceStop]),
            "{events:?}"
        );
        assert_eq!(stopped_at, Some(SILENCE_STOP));
    }

    #[test]
    fn the_silence_clock_restarts_at_each_phrase() {
        let audio = Gen::new()
            .silence(50_000)
            .tone(1000, -20.0, None)
            .silence(50_000);
        let events = run(&audio.samples, 1600);
        assert!(
            matches!(events.as_slice(), [Event::Phrase(_)]),
            "{events:?}"
        );
    }

    #[test]
    fn ten_minutes_stops_the_take_and_keeps_the_last_phrase() {
        let mut audio = Gen::new().silence(500);
        // Speech every 20 s so the silence stop never fires.
        while audio.samples.len() < TAKE_LIMIT + samples(5_000) {
            audio = audio.tone(2_000, -20.0, None).silence(18_000);
        }
        // Speech running into the limit.
        let mut audio = audio.samples;
        audio.truncate(TAKE_LIMIT - samples(1_000));
        audio.extend(Gen::new().tone(3_000, -20.0, None).samples);
        let events = run(&audio, 16_000);
        let last_two: Vec<_> = events.iter().rev().take(2).collect();
        assert!(matches!(last_two[0], Event::LimitStop));
        assert!(matches!(last_two[1], Event::Phrase(_)));
        assert_eq!(phrase_lengths(&events).len(), 31);
    }

    #[test]
    fn clicks_are_not_phrases() {
        let audio = Gen::new().silence(500).tone(50, -10.0, None).silence(1000);
        assert!(phrase_lengths(&run(&audio.samples, 160)).is_empty());
    }

    #[test]
    fn finish_emits_the_open_phrase_and_discard_drops_it() {
        let audio = Gen::new().silence(300).tone(800, -20.0, None);
        let mut seg = Segmenter::new();
        let mut events = Vec::new();
        seg.push(&audio.samples, &mut events);
        assert!(events.is_empty());
        seg.finish(&mut events);
        assert_eq!(phrase_lengths(&events), vec![samples(200 + 800)]);

        let mut seg = Segmenter::new();
        let mut events = Vec::new();
        seg.push(&audio.samples, &mut events);
        seg.discard();
        seg.finish(&mut events);
        assert!(events.is_empty());
    }

    #[test]
    fn every_buffer_is_wiped_after_use() {
        use crate::voice::buffer::tests::wiped_since_last;
        let audio = Gen::new()
            .silence(500)
            .tone(1000, -20.0, None)
            .silence(800)
            .tone(700, -20.0, None);
        let _ = wiped_since_last();
        let mut seg = Segmenter::new();
        let mut events = Vec::new();
        seg.push(&audio.samples, &mut events);
        seg.finish(&mut events);
        assert!(seg.frame.allocation_is_zero());
        assert!(seg.pre_roll.allocation_is_zero());
        assert!(seg.phrase.allocation_is_zero());
        let emitted: usize = phrase_lengths(&events).iter().sum();
        drop(events);
        drop(seg);
        // Every sample that went in was wiped at least once (frames,
        // pre-roll, phrases), and the emitted phrases on drop.
        assert!(wiped_since_last() >= audio.samples.len() + emitted);
    }
}
