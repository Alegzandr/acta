//! What the dictation UI shows, as plain data (components-voice.md 5):
//! the microphone button and what sits next to it, the line under the
//! editor's footer, the first-use dialog, the model list of Settings >
//! Voice, sizes and the elapsed time, and the two gestures of one key.
//! No GPUI here, so all of it is tested.

use std::time::{Duration, Instant};

use acta_core::voice::download::DownloadError;
use acta_core::voice::engine::EngineError;
use acta_core::voice::hardware::{Reason, Recommendation};
use acta_core::voice::models::{FREE_SPACE_MARGIN, ModelId};
use acta_core::voice::take::Notice;

use super::capture::CaptureError;
use crate::i18n::{Key, Locale};

/// A status shown only once it has lasted this long ("Starting...",
/// "Writing text...").
pub const STATUS_DELAY: Duration = Duration::from_millis(400);

/// Where an editor's dictation stands, as the button reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Settings > Voice > Dictation: Off. No button.
    Off,
    /// No engine chosen yet, or the chosen model is not on disk.
    NotSetUp,
    /// The model is coming: whole percent.
    Downloading(u8),
    Idle,
    Starting {
        since: Instant,
    },
    Listening {
        /// dBFS of the last 50 ms.
        level: f32,
        elapsed: Duration,
    },
    Writing {
        since: Instant,
    },
}

/// The button's look (components-voice.md 5, Microphone button).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// `mic` in `text.secondary`, hover `fill.hover`.
    Mic,
    /// `square` in `text.primary` on `fill.selected`.
    Stop,
    /// `mic` in `text.disabled`, not pressable.
    Busy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    /// "Dictate" with the shortcut.
    Dictate,
    /// "Dictate..." (it asks first).
    Setup,
    /// "Dictation is downloading its model: {percent}".
    Downloading(u8),
    /// "Stop dictating" with the shortcut.
    Stop,
}

/// What sits right of the button in the footer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    None,
    /// "Starting..." in `text.tertiary`.
    Starting,
    /// The level bar, "Listening", the elapsed time.
    Listening {
        fraction: f32,
        elapsed: Duration,
    },
    /// "Writing text..." in `text.tertiary`.
    Writing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mic {
    pub look: Look,
    pub tip: Option<Tip>,
    pub status: Status,
}

/// The button for `phase`; `None` when there is none (Off).
pub fn mic(phase: Phase, now: Instant) -> Option<Mic> {
    let after_delay = |since: Instant| now.saturating_duration_since(since) >= STATUS_DELAY;
    let m = match phase {
        Phase::Off => return None,
        Phase::NotSetUp => Mic {
            look: Look::Mic,
            tip: Some(Tip::Setup),
            status: Status::None,
        },
        Phase::Downloading(p) => Mic {
            look: Look::Mic,
            tip: Some(Tip::Downloading(p)),
            status: Status::None,
        },
        Phase::Idle => Mic {
            look: Look::Mic,
            tip: Some(Tip::Dictate),
            status: Status::None,
        },
        Phase::Starting { since } => Mic {
            look: Look::Stop,
            tip: Some(Tip::Stop),
            status: if after_delay(since) {
                Status::Starting
            } else {
                Status::None
            },
        },
        Phase::Listening { level, elapsed } => Mic {
            look: Look::Stop,
            tip: Some(Tip::Stop),
            status: Status::Listening {
                fraction: acta_core::voice::level::bar_fraction(level),
                elapsed,
            },
        },
        Phase::Writing { since } => Mic {
            look: Look::Busy,
            tip: None,
            status: if after_delay(since) {
                Status::Writing
            } else {
                Status::None
            },
        },
    };
    Some(m)
}

/// The elapsed time of a take: `0:12`, `1:05`, `10:00`.
pub fn elapsed(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

/// The line under an editor's footer (components-voice.md 5, Errors, and
/// the notices of Taking dictation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Capture(CaptureError),
    Engine(EngineError),
    Notice(Notice),
}

/// How the line is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// `text.primary` with `triangle-alert` in `danger`.
    Error,
    /// `text.tertiary` with `info`.
    Info,
    /// `text.tertiary`, no icon.
    Quiet,
}

/// The line's one ghost action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Windows: `ms-settings:privacy-microphone`; macOS: System Settings.
    OpenPrivacy,
    VoiceSettings,
    DownloadAgain,
    TryAgain,
}

impl Action {
    pub fn key(self, macos: bool) -> Key {
        match self {
            Action::OpenPrivacy if macos => Key::VoiceErrorOpenSystemSettings,
            Action::OpenPrivacy => Key::VoiceErrorOpenPrivacy,
            Action::VoiceSettings => Key::VoiceSettingsLink,
            Action::DownloadAgain => Key::VoiceErrorDownloadAgain,
            Action::TryAgain => Key::CommonTryAgain,
        }
    }
}

/// The line's text, tone and action. `linux`: Linux has no privacy page
/// to open.
pub fn line(l: Line, linux: bool) -> (Key, Tone, Option<Action>) {
    match l {
        Line::Capture(CaptureError::Blocked) => (
            Key::VoiceErrorBlocked,
            Tone::Error,
            (!linux).then_some(Action::OpenPrivacy),
        ),
        Line::Capture(CaptureError::NoDevice) => (
            Key::VoiceErrorNoDevice,
            Tone::Error,
            Some(Action::VoiceSettings),
        ),
        Line::Capture(CaptureError::DeviceLost) => (
            Key::VoiceErrorDeviceLost,
            Tone::Error,
            Some(Action::VoiceSettings),
        ),
        Line::Engine(EngineError::Model) => (
            Key::VoiceErrorModel,
            Tone::Error,
            Some(Action::DownloadAgain),
        ),
        Line::Engine(_) => (Key::VoiceErrorEngine, Tone::Error, Some(Action::TryAgain)),
        Line::Notice(Notice::NothingHeard) => (
            Key::VoiceNothingHeard,
            Tone::Info,
            Some(Action::VoiceSettings),
        ),
        Line::Notice(Notice::StoppedSilence) => (Key::VoiceStoppedSilence, Tone::Quiet, None),
        Line::Notice(Notice::StoppedLimit) => (Key::VoiceStoppedLimit, Tone::Quiet, None),
    }
}

/// A size as the file managers show it (components-attachments.md 5):
/// bytes under 1 KB, then KB, MB and GB of 1,024 with one decimal under
/// 10 and none above. "812 B", "2.1 MB", "190 MB"; French "2,1 Mo".
pub fn size(locale: Locale, bytes: u64) -> String {
    let units: [&str; 4] = match locale {
        Locale::En => ["B", "KB", "MB", "GB"],
        Locale::Fr => ["o", "Ko", "Mo", "Go"],
    };
    if bytes < 1024 {
        return format!("{bytes} {}", units[0]);
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 1;
    while value >= 1024.0 && unit < 3 {
        value /= 1024.0;
        unit += 1;
    }
    let tenths = (value * 10.0).round() / 10.0;
    let text = if tenths < 10.0 {
        let s = format!("{tenths:.1}");
        if locale == Locale::Fr {
            s.replace('.', ",")
        } else {
            s
        }
    } else {
        format!("{}", value.round() as u64)
    };
    format!("{text} {}", units[unit])
}

/// Whole percent of `done` over `total`.
pub fn percent_of(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((done.min(total) as f64 / total as f64) * 100.0).floor() as u8
}

// ---- the first-use dialog ----

/// What the first-use dialog shows (components-voice.md 5, First use).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setup {
    /// The recommendation, with the free space; `enough` false disables
    /// Download and shows the warning.
    Offer {
        model: ModelId,
        gpu: bool,
        free: Option<u64>,
        enough: bool,
    },
    /// Under the hardware floor: the reason and Close only.
    TooLittleMemory {
        memory: u64,
    },
    /// A processor this build's Whisper cannot run on (no AVX2 and FMA).
    UnsupportedCpu,
    /// The machine is still being looked at (a few milliseconds).
    Detecting,
    Downloading {
        model: ModelId,
        done: u64,
        total: u64,
    },
    Checking {
        model: ModelId,
    },
    Ready,
    Failed {
        model: ModelId,
        error: DownloadError,
    },
}

/// Whether `free` bytes can take `model` and still leave 1 GB.
pub fn fits(model: ModelId, free: u64) -> bool {
    free >= model.file().size + FREE_SPACE_MARGIN
}

/// The offer for a recommendation (before any download). `chosen`: a
/// model already picked in Settings whose file is missing is offered
/// instead of the recommendation.
pub fn offer(
    rec: Option<Recommendation>,
    memory: u64,
    chosen: Option<ModelId>,
    free: Option<u64>,
) -> Setup {
    let Some(rec) = rec else {
        return Setup::Detecting;
    };
    let model = match (chosen, rec.model) {
        (Some(m), _) if rec.reason != Reason::UnsupportedCpu => m,
        (_, Some(m)) => m,
        (_, None) => {
            return match rec.reason {
                Reason::UnsupportedCpu => Setup::UnsupportedCpu,
                _ => Setup::TooLittleMemory { memory },
            };
        }
    };
    Setup::Offer {
        model,
        gpu: rec.reason == Reason::Gpu && model == ModelId::LargeTurbo,
        free,
        enough: free.is_none_or(|f| fits(model, f)),
    }
}

/// A model's name as shown ("Small", "Large turbo"): names, the same in
/// both languages.
pub fn model_name(m: ModelId) -> &'static str {
    match m {
        ModelId::Tiny => "Tiny",
        ModelId::Base => "Base",
        ModelId::Small => "Small",
        ModelId::LargeTurbo => "Large turbo",
    }
}

/// The model list's hint key for a model.
pub fn model_hint(m: ModelId) -> Key {
    match m {
        ModelId::Tiny => Key::VoiceModelTinyHint,
        ModelId::Base => Key::VoiceModelBaseHint,
        ModelId::Small => Key::VoiceModelSmallHint,
        ModelId::LargeTurbo => Key::VoiceModelTurboHint,
    }
}

/// The reason line of a failed download: the key and its `{reason}` key.
pub fn download_failure(e: DownloadError) -> (Key, Option<Key>) {
    match e {
        DownloadError::Mismatch => (Key::VoiceDownloadFailed, Some(Key::VoiceDownloadMismatch)),
        DownloadError::Network | DownloadError::Status(_) => {
            (Key::VoiceDownloadFailed, Some(Key::VoiceDownloadOffline))
        }
        DownloadError::NoSpace { .. } => (Key::VoiceSetupNoSpace, None),
        _ => (Key::VoiceDownloadFailed, None),
    }
}

// ---- Settings > Voice, the model list ----

/// A model's state at the right of its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// Ghost "Download".
    NotDownloaded,
    /// The bar, the percent, ghost "Cancel".
    Downloading(u8),
    /// `check` and "In use", the menu.
    InUse,
    /// Ghost "Use", the menu.
    Downloaded,
}

/// A row's state from what is on disk, what is chosen and what downloads.
pub fn row_state(
    model: ModelId,
    on_disk: bool,
    chosen: Option<ModelId>,
    downloading: Option<(ModelId, u8)>,
) -> RowState {
    match downloading {
        Some((m, p)) if m == model => RowState::Downloading(p),
        _ if !on_disk => RowState::NotDownloaded,
        _ if chosen == Some(model) => RowState::InUse,
        _ => RowState::Downloaded,
    }
}

// ---- one key, two gestures ----

/// A press held this long is push-to-talk.
pub const HOLD: Duration = Duration::from_millis(300);
/// Key-downs closer than this to the previous one are the key repeating
/// while held, not a new press.
const REPEAT_GAP: Duration = Duration::from_millis(700);

/// What a key (or mouse) event asks of the take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Start,
    Stop,
    Nothing,
}

struct Press {
    at: Instant,
    last: Instant,
    /// This press started the take (only such a press can be a hold).
    started: bool,
}

/// `Mod+Shift+Space` or the button (components-voice.md 5): a short press
/// toggles; a press held 300 ms or more is push-to-talk, and its release
/// stops.
#[derive(Default)]
pub struct Gestures {
    press: Option<Press>,
}

impl Gestures {
    /// The key went down. `running`: a take is starting or listening in
    /// this editor.
    pub fn down(&mut self, now: Instant, running: bool) -> Gesture {
        if let Some(p) = &mut self.press
            && now.saturating_duration_since(p.last) < REPEAT_GAP
        {
            p.last = now;
            return Gesture::Nothing;
        }
        self.press = Some(Press {
            at: now,
            last: now,
            started: !running,
        });
        if running {
            Gesture::Stop
        } else {
            Gesture::Start
        }
    }

    /// The key went up.
    pub fn up(&mut self, now: Instant, running: bool) -> Gesture {
        match self.press.take() {
            Some(p) if p.started && running && now.saturating_duration_since(p.at) >= HOLD => {
                Gesture::Stop
            }
            _ => Gesture::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_follows_a_take() {
        let t0 = Instant::now();
        let soon = t0 + Duration::from_millis(100);
        let later = t0 + Duration::from_millis(500);
        assert_eq!(mic(Phase::Off, t0), None);
        let idle = mic(Phase::Idle, t0).unwrap();
        assert_eq!(
            (idle.look, idle.tip, idle.status),
            (Look::Mic, Some(Tip::Dictate), Status::None)
        );
        assert_eq!(mic(Phase::NotSetUp, t0).unwrap().tip, Some(Tip::Setup));
        assert_eq!(
            mic(Phase::Downloading(45), t0).unwrap().tip,
            Some(Tip::Downloading(45))
        );
        // "Starting..." and "Writing text..." only after 400 ms.
        let starting = mic(Phase::Starting { since: t0 }, soon).unwrap();
        assert_eq!(
            (starting.look, starting.tip, starting.status),
            (Look::Stop, Some(Tip::Stop), Status::None)
        );
        assert_eq!(
            mic(Phase::Starting { since: t0 }, later).unwrap().status,
            Status::Starting
        );
        let writing = mic(Phase::Writing { since: t0 }, soon).unwrap();
        assert_eq!(
            (writing.look, writing.tip, writing.status),
            (Look::Busy, None, Status::None)
        );
        assert_eq!(
            mic(Phase::Writing { since: t0 }, later).unwrap().status,
            Status::Writing
        );
        let listening = mic(
            Phase::Listening {
                level: -30.0,
                elapsed: Duration::from_secs(12),
            },
            t0,
        )
        .unwrap();
        assert_eq!(listening.look, Look::Stop);
        assert_eq!(
            listening.status,
            Status::Listening {
                fraction: 0.5,
                elapsed: Duration::from_secs(12)
            }
        );
    }

    #[test]
    fn elapsed_reads_minutes_and_seconds() {
        assert_eq!(elapsed(Duration::from_millis(12_900)), "0:12");
        assert_eq!(elapsed(Duration::from_secs(65)), "1:05");
        assert_eq!(elapsed(Duration::from_secs(600)), "10:00");
    }

    #[test]
    fn every_line_has_its_key_tone_and_action() {
        use Action::*;
        let table = [
            (
                Line::Capture(CaptureError::Blocked),
                Key::VoiceErrorBlocked,
                Tone::Error,
                Some(OpenPrivacy),
            ),
            (
                Line::Capture(CaptureError::NoDevice),
                Key::VoiceErrorNoDevice,
                Tone::Error,
                Some(VoiceSettings),
            ),
            (
                Line::Capture(CaptureError::DeviceLost),
                Key::VoiceErrorDeviceLost,
                Tone::Error,
                Some(VoiceSettings),
            ),
            (
                Line::Engine(EngineError::Model),
                Key::VoiceErrorModel,
                Tone::Error,
                Some(DownloadAgain),
            ),
            (
                Line::Engine(EngineError::Exited),
                Key::VoiceErrorEngine,
                Tone::Error,
                Some(TryAgain),
            ),
            (
                Line::Notice(Notice::NothingHeard),
                Key::VoiceNothingHeard,
                Tone::Info,
                Some(VoiceSettings),
            ),
            (
                Line::Notice(Notice::StoppedSilence),
                Key::VoiceStoppedSilence,
                Tone::Quiet,
                None,
            ),
            (
                Line::Notice(Notice::StoppedLimit),
                Key::VoiceStoppedLimit,
                Tone::Quiet,
                None,
            ),
        ];
        for (l, key, tone, action) in table {
            assert_eq!(line(l, false), (key, tone, action), "{l:?}");
        }
        // Linux has no privacy page to open.
        assert_eq!(line(Line::Capture(CaptureError::Blocked), true).2, None);
        assert_eq!(OpenPrivacy.key(true), Key::VoiceErrorOpenSystemSettings);
        assert_eq!(OpenPrivacy.key(false), Key::VoiceErrorOpenPrivacy);
    }

    #[test]
    fn sizes_follow_the_file_managers() {
        assert_eq!(size(Locale::En, 812), "812 B");
        assert_eq!(size(Locale::Fr, 812), "812 o");
        assert_eq!(size(Locale::En, 2_202_009), "2.1 MB");
        assert_eq!(size(Locale::Fr, 2_202_009), "2,1 Mo");
        assert_eq!(size(Locale::En, 32_152_673), "31 MB");
        assert_eq!(size(Locale::En, 190_085_487), "181 MB");
        assert_eq!(size(Locale::En, 574_041_195), "547 MB");
        assert_eq!(size(Locale::Fr, 120_259_084_288), "112 Go");
        assert_eq!(percent_of(45, 100), 45);
        assert_eq!(percent_of(0, 0), 0);
        assert_eq!(percent_of(200, 100), 100);
    }

    #[test]
    fn the_dialog_offers_the_recommendation_or_says_why_not() {
        let gb = 1_000_000_000;
        let cpu = Recommendation {
            model: Some(ModelId::Small),
            reason: Reason::Cpu,
        };
        assert_eq!(offer(None, 0, None, None), Setup::Detecting);
        assert_eq!(
            offer(Some(cpu), 16 * gb, None, Some(112 * gb)),
            Setup::Offer {
                model: ModelId::Small,
                gpu: false,
                free: Some(112 * gb),
                enough: true
            }
        );
        // Under the model plus 1 GB: Download disabled.
        let tight = ModelId::Small.file().size + FREE_SPACE_MARGIN - 1;
        assert!(matches!(
            offer(Some(cpu), 16 * gb, None, Some(tight)),
            Setup::Offer { enough: false, .. }
        ));
        let gpu = Recommendation {
            model: Some(ModelId::LargeTurbo),
            reason: Reason::Gpu,
        };
        assert!(matches!(
            offer(Some(gpu), 16 * gb, None, Some(100 * gb)),
            Setup::Offer { gpu: true, .. }
        ));
        // A chosen model whose file is missing is offered instead.
        assert!(matches!(
            offer(Some(gpu), 16 * gb, Some(ModelId::Base), Some(100 * gb)),
            Setup::Offer {
                model: ModelId::Base,
                gpu: false,
                ..
            }
        ));
        let none = Recommendation {
            model: None,
            reason: Reason::TooLittleMemory,
        };
        assert_eq!(
            offer(Some(none), gb, None, None),
            Setup::TooLittleMemory { memory: gb }
        );
        let old = Recommendation {
            model: None,
            reason: Reason::UnsupportedCpu,
        };
        assert_eq!(
            offer(Some(old), 16 * gb, Some(ModelId::Small), None),
            Setup::UnsupportedCpu
        );
    }

    #[test]
    fn failed_downloads_say_why() {
        assert_eq!(
            download_failure(DownloadError::Mismatch),
            (Key::VoiceDownloadFailed, Some(Key::VoiceDownloadMismatch))
        );
        assert_eq!(
            download_failure(DownloadError::Network),
            (Key::VoiceDownloadFailed, Some(Key::VoiceDownloadOffline))
        );
        assert_eq!(
            download_failure(DownloadError::NoSpace { needed: 1, free: 0 }).0,
            Key::VoiceSetupNoSpace
        );
        assert_eq!(
            download_failure(DownloadError::Cancelled),
            (Key::VoiceDownloadFailed, None)
        );
    }

    #[test]
    fn each_model_row_shows_its_state() {
        use ModelId::*;
        assert_eq!(
            row_state(Tiny, false, Some(Small), None),
            RowState::NotDownloaded
        );
        assert_eq!(row_state(Small, true, Some(Small), None), RowState::InUse);
        assert_eq!(
            row_state(Base, true, Some(Small), None),
            RowState::Downloaded
        );
        assert_eq!(
            row_state(LargeTurbo, false, Some(Small), Some((LargeTurbo, 45))),
            RowState::Downloading(45)
        );
        assert_eq!(
            row_state(Base, true, None, Some((LargeTurbo, 45))),
            RowState::Downloaded
        );
    }

    #[test]
    fn a_short_press_toggles_and_a_held_one_talks_until_release() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        // Tap: start, release early: still running; tap again: stop.
        let mut g = Gestures::default();
        assert_eq!(g.down(t0, false), Gesture::Start);
        assert_eq!(g.up(ms(120), true), Gesture::Nothing);
        assert_eq!(g.down(ms(2_000), true), Gesture::Stop);
        assert_eq!(g.up(ms(2_100), false), Gesture::Nothing);
        // Hold: start, the key repeats, release after 300 ms stops.
        let mut g = Gestures::default();
        assert_eq!(g.down(t0, false), Gesture::Start);
        for n in [500, 533, 566, 900, 1_200] {
            assert_eq!(g.down(ms(n), true), Gesture::Nothing, "repeat at {n}");
        }
        assert_eq!(g.up(ms(1_250), true), Gesture::Stop);
        // A hold whose take already ended (an error) stops nothing.
        let mut g = Gestures::default();
        g.down(t0, false);
        assert_eq!(g.up(ms(800), false), Gesture::Nothing);
        // A key-up that was lost (focus left) does not swallow the next
        // press made well after.
        let mut g = Gestures::default();
        g.down(t0, false);
        assert_eq!(g.down(ms(3_000), true), Gesture::Stop);
        // Holding the key that stops a take never restarts it.
        assert_eq!(g.up(ms(3_600), false), Gesture::Nothing);
    }
}
