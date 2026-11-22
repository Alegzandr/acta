//! Microphone capture for dictation (ADR 0014, 5): the device's stream
//! into a lock-free ring on the audio thread; a capture thread that
//! downmixes, resamples to 16 kHz, measures the level and cuts phrases.
//!
//! Audio exists only in memory (ADR 0014, 7): the ring's slots are zeroed
//! when it is dropped, every buffer of samples here is an [`AudioBuf`], the
//! resampler is reset at the end of a take, and nothing is logged but
//! states and error kinds.
//!
//! The capture thread is driven by whatever fills the ring: cpal in the
//! app, a generated signal in tests and in debug builds' dictation hook
//! ([`start_synthetic`]); no test opens a device.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;

use acta_core::voice::SAMPLE_RATE;
use acta_core::voice::buffer::AudioBuf;
use acta_core::voice::level::LevelMeter;
use acta_core::voice::segment::{Event as SegmentEvent, Segmenter};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

// ---- the ring ----

/// A single-producer single-consumer ring of samples. The producer side
/// runs on the audio thread: atomics only, no lock, no allocation.
struct Ring {
    slots: Box<[AtomicU32]>,
    /// Samples written and read since the start; positions modulo the
    /// capacity.
    written: AtomicUsize,
    read: AtomicUsize,
    /// Samples the producer had to drop because the ring was full.
    dropped: AtomicUsize,
}

impl Drop for Ring {
    fn drop(&mut self) {
        for slot in self.slots.iter() {
            slot.store(0, Ordering::Relaxed);
        }
    }
}

pub struct Producer(Arc<Ring>);
pub struct Consumer(Arc<Ring>);

/// A ring of `capacity` samples.
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let ring = Arc::new(Ring {
        slots: (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect(),
        written: AtomicUsize::new(0),
        read: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    (Producer(Arc::clone(&ring)), Consumer(ring))
}

impl Producer {
    /// Appends one sample; a full ring drops it.
    pub fn push(&self, sample: f32) {
        let ring = &*self.0;
        let written = ring.written.load(Ordering::Relaxed);
        let read = ring.read.load(Ordering::Acquire);
        if written - read >= ring.slots.len() {
            ring.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        ring.slots[written % ring.slots.len()].store(sample.to_bits(), Ordering::Relaxed);
        ring.written.store(written + 1, Ordering::Release);
    }

    pub fn push_slice(&self, samples: &[f32]) {
        for s in samples {
            self.push(*s);
        }
    }
}

impl Consumer {
    /// Moves up to `max` samples, a multiple of `multiple`, into `out`;
    /// each slot read is zeroed. Returns how many.
    pub fn pop_into(&self, out: &mut AudioBuf, max: usize, multiple: usize) -> usize {
        let ring = &*self.0;
        let read = ring.read.load(Ordering::Relaxed);
        let written = ring.written.load(Ordering::Acquire);
        let available = (written - read).min(max);
        let n = available - available % multiple.max(1);
        for i in 0..n {
            let slot = &ring.slots[(read + i) % ring.slots.len()];
            out.push(f32::from_bits(slot.swap(0, Ordering::Relaxed)));
        }
        ring.read.store(read + n, Ordering::Release);
        n
    }

    /// Samples the producer dropped so far.
    #[cfg(test)]
    pub fn dropped(&self) -> usize {
        self.0.dropped.load(Ordering::Relaxed)
    }
}

// ---- downmix and resample ----

/// Samples per resampler call on the input side.
const CHUNK: usize = 1024;

/// Interleaved frames at the device's rate and channel count in, 16 kHz
/// mono out: channels averaged, then resampled (rubato's FFT resampler,
/// fixed ratio). The resampler's latency is skipped at the start and
/// [`Pipeline::flush`] pads the end, so the output is as long as the
/// input at the new rate.
pub struct Pipeline {
    channels: usize,
    resampler: Option<Fft<f32>>,
    /// Mono samples waiting for a full resampler chunk.
    pending: AudioBuf,
    /// The resampler's output scratch, wiped after each call.
    scratch: Vec<f32>,
    /// Output frames still to skip (the resampler's delay).
    skip: usize,
    /// Mono frames taken in, frames given out.
    frames_in: u64,
    frames_out: u64,
    ratio: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnsupportedFormat;

impl Pipeline {
    pub fn new(rate: u32, channels: u16) -> Result<Self, UnsupportedFormat> {
        if rate == 0 || channels == 0 {
            return Err(UnsupportedFormat);
        }
        let (resampler, scratch, skip) = if rate as usize == SAMPLE_RATE {
            (None, Vec::new(), 0)
        } else {
            let r = Fft::<f32>::new(rate as usize, SAMPLE_RATE, CHUNK, 1, FixedSync::Input)
                .map_err(|_| UnsupportedFormat)?;
            let scratch = vec![0.0; r.output_frames_max()];
            let skip = r.output_delay();
            (Some(r), scratch, skip)
        };
        Ok(Pipeline {
            channels: channels as usize,
            resampler,
            pending: AudioBuf::with_capacity(CHUNK * 2),
            scratch,
            skip,
            frames_in: 0,
            frames_out: 0,
            ratio: SAMPLE_RATE as f64 / rate as f64,
        })
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Takes whole interleaved frames; appends 16 kHz mono to `out`.
    pub fn push(&mut self, interleaved: &[f32], out: &mut AudioBuf) {
        let ch = self.channels;
        for frame in interleaved.chunks_exact(ch) {
            let mono = frame.iter().sum::<f32>() / ch as f32;
            self.frames_in += 1;
            if self.resampler.is_none() {
                out.push(mono);
                self.frames_out += 1;
            } else {
                self.pending.push(mono);
            }
        }
        while self.resampler.is_some() && self.pending.len() >= CHUNK {
            self.run_chunk(out);
        }
    }

    fn run_chunk(&mut self, out: &mut AudioBuf) {
        let Some(r) = self.resampler.as_mut() else {
            return;
        };
        let needed = r.input_frames_next();
        let Ok(input) = InterleavedSlice::new(&self.pending.as_slice()[..needed], 1, needed) else {
            return;
        };
        let frames = r.output_frames_next();
        let Ok(mut output) = InterleavedSlice::new_mut(&mut self.scratch[..frames], 1, frames)
        else {
            return;
        };
        let written = match r.process_into_buffer(&input, &mut output, None) {
            Ok((_, written)) => written,
            Err(_) => 0,
        };
        let skipped = self.skip.min(written);
        self.skip -= skipped;
        out.extend_from_slice(&self.scratch[skipped..written]);
        self.frames_out += (written - skipped) as u64;
        self.scratch[..written].fill(0.0);
        self.pending.drain_front(needed);
    }

    /// The input is over: what is still inside the resampler comes out,
    /// trimmed to the length the input has at 16 kHz.
    pub fn flush(&mut self, out: &mut AudioBuf) {
        if self.resampler.is_none() {
            return;
        }
        let expected = (self.frames_in as f64 * self.ratio).round() as u64;
        // Silence pushes the last real samples through the filter.
        let mut guard = 0;
        while self.frames_out < expected && guard < 64 {
            while self.pending.len() < CHUNK {
                self.pending.push(0.0);
            }
            self.run_chunk(out);
            guard += 1;
        }
        let extra = self.frames_out.saturating_sub(expected) as usize;
        if extra > 0 {
            // Drop the padding's own output from the end.
            let keep = out.len() - extra.min(out.len());
            let mut tail = AudioBuf::with_capacity(out.len());
            tail.extend_from_slice(&out.as_slice()[..keep]);
            *out = tail;
            self.frames_out = expected;
        }
        self.wipe();
    }

    /// Forgets everything held, zeroing it.
    pub fn wipe(&mut self) {
        self.pending.wipe();
        self.scratch.fill(0.0);
        if let Some(r) = self.resampler.as_mut() {
            r.reset();
        }
    }
}

// ---- errors ----

/// What the editor's line says (components-voice.md 5, Errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// The system blocks the microphone (`voice.error.blocked`).
    Blocked,
    /// No microphone, or it could not be opened (`voice.error.no_device`).
    NoDevice,
    /// The microphone went away during a take (`voice.error.device_lost`).
    DeviceLost,
}

/// `E_ACCESSDENIED` as cpal's WASAPI backend reports it: an unclassified
/// backend error whose message ends with the HRESULT as a signed decimal
/// ("... (os error -2147024891)", windows-result 0.4).
const E_ACCESSDENIED: &str = "-2147024891";

/// The editor's line for a host error, or `None` when the error is not
/// fatal (an overrun, a route change the stream followed). `started`: the
/// stream was running when it came (a device lost) rather than failing to
/// start.
pub fn map_error(
    kind: cpal::ErrorKind,
    message: Option<&str>,
    started: bool,
) -> Option<CaptureError> {
    use cpal::ErrorKind as K;
    let denied = message.is_some_and(|m| m.contains(E_ACCESSDENIED) || m.contains("0x80070005"));
    let lost_or = |at_start| {
        if started {
            CaptureError::DeviceLost
        } else {
            at_start
        }
    };
    match kind {
        K::Xrun | K::RealtimeDenied | K::DeviceChanged => None,
        K::PermissionDenied => Some(CaptureError::Blocked),
        K::BackendError | K::Other if denied => Some(CaptureError::Blocked),
        _ => Some(lost_or(CaptureError::NoDevice)),
    }
}

// ---- the capture thread ----

/// What the capture thread tells the take, in order.
#[derive(Debug)]
pub enum Event {
    /// The device delivers audio.
    Started,
    /// The level of the last 50 ms, in dBFS.
    Level(f32),
    /// A phrase for the engine.
    Phrase(AudioBuf),
    /// 60 s without speech: the take stops.
    SilenceStop,
    /// 10 minutes: the take stops.
    LimitStop,
    Error(CaptureError),
    /// The thread is done; nothing follows.
    Ended,
}

const RUN: u8 = 0;
const STOP: u8 = 1;
const DISCARD: u8 = 2;

/// Shared between the capture thread, its handle and the device's error
/// callback.
#[derive(Default)]
struct Control {
    /// [`RUN`], [`STOP`] (finish: the last phrase still comes out) or
    /// [`DISCARD`] (drop what is not emitted).
    mode: AtomicU8,
    /// Set by the stream's error callback: the device went away.
    lost: AtomicBool,
}

/// The running capture of one take.
pub struct Handle {
    control: Arc<Control>,
    _thread: Option<JoinHandle<()>>,
}

impl Handle {
    /// The take ends: the open phrase is emitted, then [`Event::Ended`].
    pub fn stop(&self) {
        let _ = self
            .control
            .mode
            .compare_exchange(RUN, STOP, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// `Esc`: drop what has not been emitted, then [`Event::Ended`].
    pub fn discard(&self) {
        self.control.mode.store(DISCARD, Ordering::SeqCst);
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.discard();
    }
}

/// How often the capture thread wakes.
const TICK: Duration = Duration::from_millis(10);

/// The capture loop over a ring fed elsewhere, until it is stopped,
/// discarded or the device is lost; every buffer is wiped on the way out.
fn run(consumer: Consumer, mut pipeline: Pipeline, control: &Control, events: &Sender<Event>) {
    let mut segmenter = Segmenter::new();
    let mut meter = LevelMeter::new();
    let mut raw = AudioBuf::with_capacity(16_384);
    let mut mono = AudioBuf::with_capacity(8_192);
    let mut cut = Vec::new();
    let channels = pipeline.channels();
    let send_cut = |cut: &mut Vec<SegmentEvent>| -> bool {
        let mut over = false;
        for e in cut.drain(..) {
            let event = match e {
                SegmentEvent::Phrase(p) => Event::Phrase(p),
                SegmentEvent::SilenceStop => {
                    over = true;
                    Event::SilenceStop
                }
                SegmentEvent::LimitStop => {
                    over = true;
                    Event::LimitStop
                }
            };
            let _ = events.send(event);
        }
        over
    };
    loop {
        let mode = control.mode.load(Ordering::SeqCst);
        if mode == DISCARD {
            segmenter.discard();
            break;
        }
        let lost = control.lost.load(Ordering::SeqCst);
        // Everything the device delivered, in whole frames.
        while consumer.pop_into(&mut raw, 8_192, channels) > 0 {
            pipeline.push(raw.as_slice(), &mut mono);
            raw.wipe();
        }
        let ending = mode == STOP || lost;
        if ending {
            pipeline.flush(&mut mono);
        }
        if !mono.is_empty() {
            if let Some(level) = meter.push(mono.as_slice()) {
                let _ = events.send(Event::Level(level));
            }
            segmenter.push(mono.as_slice(), &mut cut);
            mono.wipe();
            if send_cut(&mut cut) {
                break;
            }
        }
        if ending {
            segmenter.finish(&mut cut);
            send_cut(&mut cut);
            if lost {
                let _ = events.send(Event::Error(CaptureError::DeviceLost));
            }
            break;
        }
        std::thread::sleep(TICK);
    }
    pipeline.wipe();
    raw.wipe();
    mono.wipe();
    // Anything still in the ring is zeroed when the ring is dropped.
    drop(consumer);
}

/// Ring capacity: two seconds of the device's audio.
fn ring_capacity(rate: u32, channels: u16) -> usize {
    rate as usize * channels as usize * 2
}

/// Starts capturing from `device` (a system id, or the default), on a
/// thread of its own that owns the stream. Events, starting with
/// [`Event::Started`] or an [`Event::Error`], go to `events`; the last is
/// always [`Event::Ended`].
pub fn start(device: Option<String>, events: Sender<Event>) -> Handle {
    let control = Arc::new(Control::default());
    let shared = Arc::clone(&control);
    let thread = std::thread::Builder::new()
        .name("acta-voice-capture".into())
        .spawn(move || {
            match open(device.as_deref(), &shared) {
                Ok((stream, consumer, pipeline)) => {
                    let _ = events.send(Event::Started);
                    run(consumer, pipeline, &shared, &events);
                    drop(stream);
                }
                Err(e) => {
                    crate::log::line(&format!("voice: the microphone did not start ({e:?})"));
                    let _ = events.send(Event::Error(e));
                }
            }
            let _ = events.send(Event::Ended);
        })
        .ok();
    Handle {
        control,
        _thread: thread,
    }
}

/// Opens the device and starts its stream into a new ring.
fn open(
    device: Option<&str>,
    control: &Arc<Control>,
) -> Result<(cpal::Stream, Consumer, Pipeline), CaptureError> {
    use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
    let host = cpal::default_host();
    let chosen = device
        .and_then(|id| id.parse::<cpal::DeviceId>().ok())
        .and_then(|id| host.device_by_id(&id));
    // A chosen microphone that is gone falls back to the default one.
    let device = chosen
        .or_else(|| host.default_input_device())
        .ok_or(CaptureError::NoDevice)?;
    let fail =
        |e: cpal::Error| map_error(e.kind(), e.message(), false).unwrap_or(CaptureError::NoDevice);
    let config = device.default_input_config().map_err(fail)?;
    let format = config.sample_format();
    let stream_config = config.config();
    let pipeline = Pipeline::new(stream_config.sample_rate, stream_config.channels)
        .map_err(|_| CaptureError::NoDevice)?;
    let (producer, consumer) = ring(ring_capacity(
        stream_config.sample_rate,
        stream_config.channels,
    ));
    let lost = Arc::clone(control);
    let on_error = move |e: cpal::Error| {
        if map_error(e.kind(), e.message(), true).is_some() {
            lost.lost.store(true, Ordering::SeqCst);
        }
    };
    let timeout = Some(Duration::from_secs(5));
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| producer.push_slice(data),
            on_error,
            timeout,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                for s in data {
                    producer.push(f32::from(*s) / 32_768.0);
                }
            },
            on_error,
            timeout,
        ),
        cpal::SampleFormat::I32 => device.build_input_stream(
            stream_config,
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                for s in data {
                    producer.push((f64::from(*s) / 2_147_483_648.0) as f32);
                }
            },
            on_error,
            timeout,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            stream_config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                for s in data {
                    producer.push((f32::from(*s) - 32_768.0) / 32_768.0);
                }
            },
            on_error,
            timeout,
        ),
        _ => return Err(CaptureError::NoDevice),
    }
    .map_err(fail)?;
    stream.play().map_err(fail)?;
    Ok((stream, consumer, pipeline))
}

/// An input device as Settings lists it: its system id (stored in the
/// database's local values) and its system name (shown, never stored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
}

/// The input devices, by their system names. Blocking (the host is
/// asked): call it off the UI thread.
pub fn devices() -> Vec<Device> {
    use cpal::traits::{DeviceTrait as _, HostTrait as _};
    let host = cpal::default_host();
    let Ok(list) = host.input_devices() else {
        return Vec::new();
    };
    list.filter_map(|d| {
        let id = d.id().ok()?.to_string();
        let name = d.description().ok()?.name().to_string();
        Some(Device { id, name })
    })
    .collect()
}

#[cfg(any(test, debug_assertions))]
/// Debug builds and tests: a take fed by a generated signal instead of a
/// device, delivered in real time at `rate` with `channels`. Nothing is
/// opened. `signal` holds interleaved samples.
pub fn start_synthetic(
    rate: u32,
    channels: u16,
    signal: AudioBuf,
    events: Sender<Event>,
) -> Handle {
    let control = Arc::new(Control::default());
    let shared = Arc::clone(&control);
    let thread = std::thread::Builder::new()
        .name("acta-voice-capture".into())
        .spawn(move || {
            let Ok(pipeline) = Pipeline::new(rate, channels) else {
                let _ = events.send(Event::Error(CaptureError::NoDevice));
                let _ = events.send(Event::Ended);
                return;
            };
            let (producer, consumer) = ring(ring_capacity(rate, channels));
            let feeder_control = Arc::clone(&shared);
            let feeder = std::thread::spawn(move || {
                let mut signal = signal;
                let per_tick = (rate as usize / 100) * channels as usize;
                let mut at = 0;
                while at < signal.len() && feeder_control.mode.load(Ordering::SeqCst) == RUN {
                    let end = (at + per_tick).min(signal.len());
                    producer.push_slice(&signal.as_slice()[at..end]);
                    at = end;
                    std::thread::sleep(TICK);
                }
                signal.wipe();
            });
            let _ = events.send(Event::Started);
            run(consumer, pipeline, &shared, &events);
            let _ = feeder.join();
            let _ = events.send(Event::Ended);
        })
        .ok();
    Handle {
        control,
        _thread: thread,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    /// `seconds` of a 440 Hz sine at `amplitude`, stereo interleaved (the
    /// same on both channels, so the downmix keeps the level).
    fn stereo_sine(rate: u32, seconds: f64, amplitude: f32) -> Vec<f32> {
        let frames = (rate as f64 * seconds) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let s = amplitude * (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin();
            out.push(s);
            out.push(s);
        }
        out
    }

    fn through_pipeline(rate: u32, input: &[f32], chunk: usize) -> AudioBuf {
        let mut p = Pipeline::new(rate, 2).unwrap();
        let mut out = AudioBuf::new();
        for part in input.chunks(chunk) {
            p.push(part, &mut out);
        }
        p.flush(&mut out);
        out
    }

    #[test]
    fn stereo_at_44_1_and_48_khz_comes_out_at_16_khz_mono_with_its_length_and_level() {
        for rate in [44_100, 48_000] {
            let input = stereo_sine(rate, 2.0, 0.5);
            // Device callbacks deliver odd sizes; whole frames only here,
            // as the ring hands them over.
            let out = through_pipeline(rate, &input, 882);
            let expected = 32_000.0;
            let len = out.len() as f64;
            assert!(
                (len - expected).abs() / expected < 0.01,
                "{rate}: {len} samples"
            );
            let want = rms(&input);
            // Away from the edges, where the filter rings.
            let got = rms(&out.as_slice()[1_600..30_400]);
            assert!(
                (got - want).abs() / want < 0.01,
                "{rate}: rms {got} vs {want}"
            );
        }
    }

    #[test]
    fn the_downmix_averages_the_channels() {
        let mut p = Pipeline::new(16_000, 2).unwrap();
        let mut out = AudioBuf::new();
        p.push(&[0.5, -0.5, 1.0, 0.0, 0.2, 0.4], &mut out);
        assert_eq!(out.as_slice(), &[0.0, 0.5, 0.3]);
    }

    #[test]
    fn sixteen_khz_input_is_not_resampled() {
        let input = stereo_sine(16_000, 0.5, 0.3);
        let out = through_pipeline(16_000, &input, 160);
        assert_eq!(out.len(), 8_000);
        assert_eq!(out.as_slice()[123], input[246]);
    }

    #[test]
    fn a_zero_rate_or_no_channel_is_refused() {
        assert!(Pipeline::new(0, 2).is_err());
        assert!(Pipeline::new(48_000, 0).is_err());
    }

    #[test]
    fn the_ring_keeps_order_drops_when_full_and_hands_out_whole_frames() {
        let (p, c) = ring(8);
        p.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let mut out = AudioBuf::new();
        assert_eq!(c.pop_into(&mut out, 100, 2), 4);
        assert_eq!(out.as_slice(), &[1.0, 2.0, 3.0, 4.0]);
        p.push_slice(&[6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0]);
        // 5 was left; 7 more fit, 13 is dropped.
        assert_eq!(c.dropped(), 1);
        let mut out = AudioBuf::new();
        assert_eq!(c.pop_into(&mut out, 100, 1), 8);
        assert_eq!(out.as_slice(), &[5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
    }

    #[test]
    fn slots_read_from_the_ring_are_zeroed() {
        let (p, c) = ring(4);
        p.push_slice(&[0.25, 0.5]);
        let mut out = AudioBuf::new();
        c.pop_into(&mut out, 4, 1);
        assert!(c.0.slots.iter().all(|s| s.load(Ordering::Relaxed) == 0));
    }

    #[test]
    fn host_errors_map_to_the_editor_lines() {
        use cpal::ErrorKind as K;
        let denied = "Access is denied. (os error -2147024891)";
        let table: [(K, Option<&str>, bool, Option<CaptureError>); 12] = [
            (
                K::PermissionDenied,
                None,
                false,
                Some(CaptureError::Blocked),
            ),
            (K::PermissionDenied, None, true, Some(CaptureError::Blocked)),
            (
                K::BackendError,
                Some(denied),
                false,
                Some(CaptureError::Blocked),
            ),
            (
                K::BackendError,
                Some("0x80070005"),
                false,
                Some(CaptureError::Blocked),
            ),
            (
                K::BackendError,
                Some("something else (os error -2004287480)"),
                false,
                Some(CaptureError::NoDevice),
            ),
            (
                K::DeviceNotAvailable,
                None,
                false,
                Some(CaptureError::NoDevice),
            ),
            (
                K::DeviceNotAvailable,
                None,
                true,
                Some(CaptureError::DeviceLost),
            ),
            (
                K::StreamInvalidated,
                None,
                true,
                Some(CaptureError::DeviceLost),
            ),
            (
                K::HostUnavailable,
                None,
                false,
                Some(CaptureError::NoDevice),
            ),
            (K::DeviceBusy, None, false, Some(CaptureError::NoDevice)),
            (K::Xrun, None, true, None),
            (K::DeviceChanged, None, true, None),
        ];
        for (kind, message, started, want) in table {
            assert_eq!(
                map_error(kind, message, started),
                want,
                "{kind:?} {message:?} {started}"
            );
        }
    }

    /// Tone bursts separated by silence, as 48 kHz stereo.
    fn bursts(rate: u32, pattern: &[(f64, f32)]) -> AudioBuf {
        let mut signal = AudioBuf::new();
        for (seconds, amplitude) in pattern {
            signal.extend_from_slice(&stereo_sine(rate, *seconds, *amplitude));
        }
        signal
    }

    fn collect(rx: &mpsc::Receiver<Event>) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(10)) {
            let end = matches!(e, Event::Ended);
            out.push(e);
            if end {
                break;
            }
        }
        out
    }

    #[test]
    fn a_fake_source_gives_started_levels_and_one_phrase_per_burst() {
        let (tx, rx) = mpsc::channel();
        let signal = bursts(
            48_000,
            &[(0.3, 0.0), (0.6, 0.3), (0.9, 0.0), (0.6, 0.3), (0.9, 0.0)],
        );
        let handle = start_synthetic(48_000, 2, signal, tx);
        // The signal is 3.3 s long; stop once it has played.
        std::thread::sleep(Duration::from_millis(3_600));
        handle.stop();
        let events = collect(&rx);
        assert!(matches!(events.first(), Some(Event::Started)));
        assert!(matches!(events.last(), Some(Event::Ended)));
        let phrases: Vec<usize> = events
            .iter()
            .filter_map(|e| match e {
                Event::Phrase(p) => Some(p.len()),
                _ => None,
            })
            .collect();
        assert_eq!(phrases.len(), 2, "{phrases:?}");
        let loud = events
            .iter()
            .filter_map(|e| match e {
                Event::Level(l) => Some(*l),
                _ => None,
            })
            .fold(f32::MIN, f32::max);
        assert!(loud > -15.0 && loud < -5.0, "{loud}");
    }

    #[test]
    fn stopping_mid_phrase_still_gives_the_phrase_and_discard_gives_nothing() {
        for discard in [false, true] {
            let (tx, rx) = mpsc::channel();
            let signal = bursts(44_100, &[(0.3, 0.0), (3.0, 0.3)]);
            let handle = start_synthetic(44_100, 2, signal, tx);
            std::thread::sleep(Duration::from_millis(1_500));
            if discard {
                handle.discard();
            } else {
                handle.stop();
            }
            let events = collect(&rx);
            let phrases = events
                .iter()
                .filter(|e| matches!(e, Event::Phrase(_)))
                .count();
            assert_eq!(phrases, usize::from(!discard), "discard: {discard}");
            assert!(matches!(events.last(), Some(Event::Ended)));
        }
    }

    #[test]
    fn a_lost_device_ends_the_take_with_what_was_heard() {
        let (tx, rx) = mpsc::channel();
        let signal = bursts(48_000, &[(0.3, 0.0), (3.0, 0.3)]);
        let handle = start_synthetic(48_000, 2, signal, tx);
        std::thread::sleep(Duration::from_millis(1_500));
        handle.control.lost.store(true, Ordering::SeqCst);
        let events = collect(&rx);
        let kinds: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                Event::Phrase(_) => Some("phrase"),
                Event::Error(CaptureError::DeviceLost) => Some("lost"),
                Event::Ended => Some("ended"),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, ["phrase", "lost", "ended"]);
    }
}
