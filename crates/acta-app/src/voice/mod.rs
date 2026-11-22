//! Dictation in the app (ADR 0014; components-voice.md 5): the settings,
//! the one take at a time across every editor, the microphone, the model
//! download and what the editors, Settings > Voice and the first-use
//! dialog read. The engine-independent rules (segmenter, anchors, the
//! take state machine, the downloader) are `acta_core::voice`; the
//! capture pipeline is [`capture`]; what is shown, as plain data, is
//! [`view`]; the elements are [`ui`].
//!
//! An editor joins with [`attach`] (one line where its `TextareaState` is
//! made), shows [`ui::mic`] in its footer and [`ui::line`] under it. Keys
//! are caught before the editors see them ([`init`]): `Mod+Shift+Space`
//! (tap toggles, hold talks), `Esc` while a take runs, and `Mod+Enter`,
//! which stops, waits for the last phrase and then lets the editor save.
//!
//! Audio and the text it becomes are never logged: log lines name states
//! and error kinds only.

pub mod capture;
#[cfg(debug_assertions)]
pub mod dev;
pub mod machine;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod settings_view;
pub mod setup;
pub mod ui;
pub mod view;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acta_core::voice::download::{self, DownloadError, ModelState, Policy, Progress};
use acta_core::voice::engine::{Engine, EngineError, Lang};
use acta_core::voice::hardware::{self, Machine, Recommendation};
use acta_core::voice::models::{self, ModelId};
use acta_core::voice::protocol::EngineSpec;
use acta_core::voice::take::{Controller, EditorId, Editors, Output, StopReason, TakeState};
use acta_core::voice::worker::WorkerClient;
use gpui_kit::component::input::{InputEditsEvent, InputEvent, TextareaState};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, Focusable as _, Global, KeyUpEvent,
    KeystrokeEvent, Subscription, Task, WeakEntity, Window, actions,
};

use self::capture::{CaptureError, Device};
use self::view::{Gesture, Gestures, Line, Phase};
use crate::settings;

actions!(
    acta_voice,
    [
        /// "Voice settings": Settings, scrolled to Voice.
        OpenVoiceSettings,
        /// The first-use dialog (components-voice.md 5, First use).
        OpenVoiceSetup,
        /// Palette: "Dictate", on the editor last focused.
        Dictate,
        /// Palette: "Stop dictating".
        StopDictating,
    ]
);

// ---- settings ----

/// Settings > Voice > Dictation. The macOS recogniser is offered only on
/// a Mac where it works on the device ([`OnDevice`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineChoice {
    Whisper,
    /// `SFSpeechRecognizer` on the device ([`macos`]); never stored on
    /// another system.
    MacOs,
    Off,
}

/// Which of Acta's languages the macOS recogniser takes without the
/// network (ADR 0014, 3). All false on Windows and Linux, and in a macOS
/// build that is not an app bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OnDevice {
    pub en: bool,
    pub fr: bool,
}

impl OnDevice {
    pub fn any(self) -> bool {
        self.en || self.fr
    }

    /// For the take's language; "detected" is the interface's language,
    /// the recogniser having no detection.
    pub fn supports(self, language: Lang, french_interface: bool) -> bool {
        match language {
            Lang::English => self.en,
            Lang::French => self.fr,
            Lang::Detect if french_interface => self.fr,
            Lang::Detect => self.en,
        }
    }

    /// Read off the UI thread (a few milliseconds on macOS).
    fn detect() -> OnDevice {
        #[cfg(target_os = "macos")]
        {
            macos::on_device()
        }
        #[cfg(not(target_os = "macos"))]
        {
            OnDevice::default()
        }
    }
}

/// What `preferences.json` holds for dictation (layout.md 4.7): settings,
/// no content. The microphone is in the database's local values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoicePrefs {
    /// `None` until the first use chose one.
    pub engine: Option<EngineChoice>,
    pub model: Option<ModelId>,
    pub language: Lang,
    /// "Use the graphics card" (on by default; shown where it applies).
    pub gpu: bool,
}

impl Default for VoicePrefs {
    fn default() -> Self {
        VoicePrefs {
            engine: None,
            model: None,
            language: Lang::Detect,
            gpu: true,
        }
    }
}

impl VoicePrefs {
    /// From the stored strings; anything unknown is the default.
    pub fn from_stored(
        engine: Option<&str>,
        model: Option<&str>,
        language: Option<&str>,
        no_gpu: bool,
    ) -> Self {
        VoicePrefs {
            engine: match engine {
                Some("whisper") => Some(EngineChoice::Whisper),
                Some("macos") if cfg!(target_os = "macos") => Some(EngineChoice::MacOs),
                Some("off") => Some(EngineChoice::Off),
                _ => None,
            },
            model: model.and_then(ModelId::from_key),
            language: match language {
                Some("en") => Lang::English,
                Some("fr") => Lang::French,
                _ => Lang::Detect,
            },
            gpu: !no_gpu,
        }
    }

    pub fn engine_key(&self) -> Option<&'static str> {
        self.engine.map(|e| match e {
            EngineChoice::Whisper => "whisper",
            EngineChoice::MacOs => "macos",
            EngineChoice::Off => "off",
        })
    }

    pub fn language_key(&self) -> Option<&'static str> {
        match self.language {
            Lang::Detect => None,
            Lang::English => Some("en"),
            Lang::French => Some("fr"),
        }
    }
}

/// The database's local value holding the chosen microphone's system id.
const MICROPHONE_KEY: &str = "voice.microphone";

// ---- the global ----

struct Handle(Entity<Dictation>);

impl Global for Handle {}

/// Creates the dictation state and catches its keys. Call once, before
/// any window opens.
pub fn init(cx: &mut App) {
    let dictation = cx.new(Dictation::new);
    cx.set_global(Handle(dictation));
    cx.intercept_keystrokes(intercept).detach();
    cx.on_action(|_: &StopDictating, cx| {
        if let Some(d) = get(cx) {
            d.update(cx, |d, cx| d.request_stop(StopReason::Asked, cx));
        }
    });
}

pub fn get(cx: &App) -> Option<Entity<Dictation>> {
    cx.try_global::<Handle>().map(|h| h.0.clone())
}

/// The database is open: the chosen microphone is read from its local
/// values.
pub fn set_db(db: crate::db::Db, cx: &mut App) {
    let Some(d) = get(cx) else { return };
    let read = db.run(cx, |store| store.local_value(MICROPHONE_KEY));
    d.update(cx, |d, _| d.db = Some(db));
    cx.spawn(async move |cx| {
        if let Ok(value) = read.await {
            d.update(cx, |d, _| d.microphone = value);
        }
    })
    .detach();
}

/// An editor that offers dictation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mic(pub EditorId);

/// Joins a multi-line editor to dictation. The editor leaves by itself
/// when its state is dropped.
pub fn attach(state: &Entity<TextareaState>, window: &mut Window, cx: &mut App) -> Mic {
    let Some(d) = get(cx) else {
        return Mic(0);
    };
    if let Some(m) = mic_of(state, cx) {
        return m;
    }
    let handle = window.window_handle();
    let id = d.update(cx, |d, cx| d.register(state, handle, cx));
    Mic(id)
}

/// The dictation handle of an editor joined with [`attach`].
pub fn mic_of(state: &Entity<TextareaState>, cx: &App) -> Option<Mic> {
    let d = get(cx)?;
    d.read(cx)
        .by_entity
        .get(&state.entity_id())
        .copied()
        .map(Mic)
}

/// `Mod+Shift+Space`, `Esc` and `Mod+Enter`, before the editors and the
/// pages see them (interaction.md 6.1).
fn intercept(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    let Some(d) = get(cx) else { return };
    let k = &event.keystroke;
    let m = k.modifiers;
    let only_secondary = m.secondary(cx) && !m.alt && !m.function && !(m.control && m.platform);
    let dictate = k.key == "space" && only_secondary && m.shift;
    let save = k.key == "enter" && only_secondary && !m.shift;
    let escape = k.key == "escape" && !m.modified();
    if !(dictate || save || escape) {
        return;
    }
    let Some(editor) = d.read(cx).focused_editor(window, cx) else {
        return;
    };
    let handled = d.update(cx, |d, cx| {
        if dictate {
            d.key_down(editor, window, cx);
            true
        } else if escape {
            d.escape(editor, cx)
        } else {
            d.save_after_take(editor, cx)
        }
    });
    if handled {
        cx.stop_propagation();
    }
}

/// The release of `Space` (push-to-talk). The shell's root listens for
/// key-ups and hands them here.
pub fn key_up(event: &KeyUpEvent, _: &mut Window, cx: &mut App) {
    if event.keystroke.key != "space" {
        return;
    }
    if let Some(d) = get(cx) {
        d.update(cx, |d, cx| d.gesture_up(cx));
    }
}

// ---- the state ----

struct Slot {
    state: WeakEntity<TextareaState>,
    window: AnyWindowHandle,
    /// Insertions of ours whose edit events are still to come: their
    /// offset and length, so they are not mistaken for typing.
    own: VecDeque<(usize, usize)>,
    _subs: Vec<Subscription>,
}

/// The capture of the take in progress.
struct Run {
    editor: EditorId,
    handle: capture::Handle,
    events: Receiver<capture::Event>,
    asked_at: Instant,
    /// When the device first delivered.
    started_at: Option<Instant>,
    /// Stop asked (the take is writing) and why.
    stop: Option<(StopReason, Instant)>,
    /// `Esc` or a dead editor: nothing more is written.
    discarding: bool,
    /// The device refused to start or went away.
    error: Option<CaptureError>,
    /// Only measures the level (Settings > Voice, Microphone).
    preview: bool,
}

/// A take waiting for the previous capture to end (its last phrase
/// belongs to the editor it was spoken in).
struct Pending {
    editor: EditorId,
    caret: usize,
    at: Instant,
}

/// What the controller's engine was made for.
#[derive(Clone, Debug, PartialEq)]
enum EngineFor {
    Whisper {
        model: ModelId,
        gpu: bool,
    },
    /// The macOS recogniser; "detected" takes the interface's language.
    MacOs {
        french_interface: bool,
    },
    #[cfg(debug_assertions)]
    Fake,
}

/// A model download in progress.
pub struct Download {
    pub model: ModelId,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    _task: Task<()>,
}

/// How the last download ended, for the first-use dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finished {
    Ready(ModelId),
    Failed(ModelId, DownloadError),
}

pub struct Dictation {
    editors: HashMap<EditorId, Slot>,
    by_entity: HashMap<gpui_kit::EntityId, EditorId>,
    next_id: EditorId,
    controller: Option<Controller>,
    engine_for: Option<EngineFor>,
    run: Option<Run>,
    pending: Option<Pending>,
    capture_errors: HashMap<EditorId, CaptureError>,
    /// When each editor's last take began writing.
    writing_since: HashMap<EditorId, Instant>,
    gestures: Gestures,
    gesture_editor: Option<EditorId>,
    /// `Mod+Enter` stopped this editor's take: it saves once written.
    save_after: Option<EditorId>,
    last_focused: Option<EditorId>,
    level: f32,
    pub machine: Option<Machine>,
    /// Where the macOS recogniser works on the device; read with the
    /// machine.
    pub system: OnDevice,
    detecting: bool,
    /// Free bytes on the models' disk.
    pub free: Option<u64>,
    on_disk: HashMap<ModelId, ModelState>,
    checking: bool,
    pub download: Option<Download>,
    pub finished: Option<Finished>,
    /// "Delete model" chosen in Settings, waiting for the confirmation.
    pub to_delete: Option<ModelId>,
    /// A refusal shown under the model list (`voice.model.busy`).
    pub refusal: Option<crate::i18n::Key>,
    pub devices: Option<Vec<Device>>,
    pub microphone: Option<String>,
    db: Option<crate::db::Db>,
    /// The Settings row's level bar was drawn on screen at this time.
    preview_seen: Option<Instant>,
    poll: Option<Task<()>>,
    #[cfg(debug_assertions)]
    pub dev: dev::Dev,
}

/// How often a running take is looked at: the level bar follows the
/// 50 ms level windows.
const TICK: Duration = Duration::from_millis(33);

impl Dictation {
    fn new(cx: &mut Context<Self>) -> Self {
        let _ = cx;
        Dictation {
            editors: HashMap::new(),
            by_entity: HashMap::new(),
            next_id: 1,
            controller: None,
            engine_for: None,
            run: None,
            pending: None,
            capture_errors: HashMap::new(),
            writing_since: HashMap::new(),
            gestures: Gestures::default(),
            gesture_editor: None,
            save_after: None,
            last_focused: None,
            level: acta_core::voice::level::FLOOR_DBFS,
            machine: None,
            system: OnDevice::default(),
            detecting: false,
            free: None,
            on_disk: HashMap::new(),
            checking: false,
            download: None,
            finished: None,
            to_delete: None,
            refusal: None,
            devices: None,
            microphone: None,
            db: None,
            preview_seen: None,
            poll: None,
            #[cfg(debug_assertions)]
            dev: dev::Dev::from_env(),
        }
    }

    fn prefs(cx: &App) -> VoicePrefs {
        settings::current(cx).voice
    }

    fn register(
        &mut self,
        state: &Entity<TextareaState>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> EditorId {
        let id = self.next_id;
        self.next_id += 1;
        let subs = vec![
            cx.subscribe(state, move |d, _, event: &InputEditsEvent, _| {
                d.edited(id, event);
            }),
            cx.subscribe(state, move |d, _, event: &InputEvent, cx| match event {
                InputEvent::Focus => d.last_focused = Some(id),
                InputEvent::Blur => d.blurred(id, cx),
                _ => {}
            }),
        ];
        self.by_entity.insert(state.entity_id(), id);
        self.editors.insert(
            id,
            Slot {
                state: state.downgrade(),
                window,
                own: VecDeque::new(),
                _subs: subs,
            },
        );
        self.last_focused = Some(id);
        // What the button shows depends on the model being on disk, or on
        // the macOS recogniser working on the device.
        self.refresh_models(false, cx);
        self.detect(cx);
        id
    }

    fn focused_editor(&self, window: &Window, cx: &App) -> Option<EditorId> {
        self.editors.iter().find_map(|(id, slot)| {
            let state = slot.state.upgrade()?;
            state
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
                .then_some(*id)
        })
    }

    // ---- what the UI reads ----

    fn set_up(&self, cx: &App) -> bool {
        #[cfg(debug_assertions)]
        if self.dev.fake.is_some() {
            return true;
        }
        let prefs = Self::prefs(cx);
        if prefs.engine == Some(EngineChoice::MacOs) {
            let french = settings::locale(cx) == crate::i18n::Locale::Fr;
            return self.system.supports(prefs.language, french);
        }
        // A model being downloaded (or downloaded again over a damaged
        // file) is not used until its download ends.
        let replacing = |m: ModelId| self.download.as_ref().is_some_and(|d| d.model == m);
        prefs.engine == Some(EngineChoice::Whisper)
            && prefs.model.is_some_and(|m| {
                !replacing(m) && matches!(self.on_disk.get(&m), Some(ModelState::Ready(_)))
            })
    }

    /// Where `editor`'s dictation stands.
    pub fn phase(&self, editor: EditorId, cx: &App) -> Phase {
        let now = Instant::now();
        #[cfg(debug_assertions)]
        if let Some(phase) = self.dev.phase(now) {
            return phase;
        }
        if Self::prefs(cx).engine == Some(EngineChoice::Off) {
            return Phase::Off;
        }
        if let Some(run) = self
            .run
            .as_ref()
            .filter(|r| r.editor == editor && !r.preview)
        {
            if run.discarding {
                return Phase::Idle;
            }
            if let Some((_, at)) = run.stop {
                return Phase::Writing { since: at };
            }
            let state = self.controller.as_ref().map(|c| c.state(editor));
            return match (run.started_at, state) {
                (Some(started), Some(TakeState::Listening)) => Phase::Listening {
                    level: self.level,
                    elapsed: now.saturating_duration_since(started),
                },
                _ => Phase::Starting {
                    since: run.asked_at,
                },
            };
        }
        if let Some(p) = self.pending.as_ref().filter(|p| p.editor == editor) {
            return Phase::Starting { since: p.at };
        }
        if let Some(TakeState::Writing) = self.controller.as_ref().map(|c| c.state(editor)) {
            return Phase::Writing {
                since: self.writing_since.get(&editor).copied().unwrap_or(now),
            };
        }
        if self.set_up(cx) {
            return Phase::Idle;
        }
        // The model this editor waits for is coming.
        match &self.download {
            Some(d) => Phase::Downloading(self.download_percent(d)),
            None => Phase::NotSetUp,
        }
    }

    /// The line under `editor`'s footer, if any.
    pub fn line(&self, editor: EditorId) -> Option<Line> {
        #[cfg(debug_assertions)]
        if let Some(line) = self.dev.line {
            return Some(line);
        }
        if let Some(e) = self.capture_errors.get(&editor) {
            return Some(Line::Capture(*e));
        }
        let controller = self.controller.as_ref()?;
        if let TakeState::Error(e) = controller.state(editor)
            && e != EngineError::Failed
        {
            return Some(Line::Engine(e));
        }
        controller.notice(editor).map(Line::Notice)
    }

    /// "Download again" or "Try again": the editor's microphone line goes.
    pub fn clear_line(&mut self, editor: EditorId) {
        self.capture_errors.remove(&editor);
    }

    /// A take is starting, listening or writing somewhere.
    pub fn running(&self) -> bool {
        self.run.as_ref().is_some_and(|r| !r.preview) || self.pending.is_some()
    }

    /// The editor "Dictate" acts on: the focused one, else the last one
    /// focused that is still open.
    pub fn target(&self) -> Option<EditorId> {
        self.last_focused.filter(|id| {
            self.editors
                .get(id)
                .is_some_and(|s| s.state.upgrade().is_some())
        })
    }

    fn download_percent(&self, d: &Download) -> u8 {
        match *d.progress.lock().unwrap_or_else(|p| p.into_inner()) {
            Progress::Bytes { done, total } => view::percent_of(done, total),
            Progress::Checking => 100,
        }
    }

    /// The download in progress: bytes done and total, or `None` while
    /// the hash is checked.
    pub fn download_progress(&self) -> Option<(ModelId, Option<(u64, u64)>)> {
        let d = self.download.as_ref()?;
        let p = *d.progress.lock().unwrap_or_else(|p| p.into_inner());
        Some((
            d.model,
            match p {
                Progress::Bytes { done, total } => Some((done, total)),
                Progress::Checking => None,
            },
        ))
    }

    pub fn recommendation(&self) -> Option<Recommendation> {
        self.machine.map(hardware::recommend)
    }

    pub fn on_disk(&self, model: ModelId) -> bool {
        matches!(self.on_disk.get(&model), Some(ModelState::Ready(_)))
    }

    /// The level of the microphone, in dBFS, for the Settings row.
    pub fn level(&self) -> f32 {
        #[cfg(debug_assertions)]
        if let Some(level) = self.dev.level {
            return level;
        }
        self.level
    }

    pub fn models_dir() -> Option<PathBuf> {
        let base = acta_core::paths::BaseDirs::from_env();
        let data = acta_core::paths::data_dir(acta_core::paths::Os::CURRENT, &base)?;
        Some(models::models_dir(&data))
    }

    /// Bytes the downloaded models take.
    pub fn models_size(&self) -> u64 {
        ModelId::ALL
            .into_iter()
            .filter(|m| self.on_disk(*m))
            .map(|m| m.file().size)
            .sum()
    }

    // ---- facts read off the UI thread ----

    /// The machine (memory, cores) and the free space, once.
    pub fn detect(&mut self, cx: &mut Context<Self>) {
        if self.machine.is_some() || self.detecting {
            return;
        }
        self.detecting = true;
        let task = cx.background_spawn(async { (machine::detect(), OnDevice::detect()) });
        cx.spawn(async move |d, cx| {
            let (m, system) = task.await;
            let _ = d.update(cx, |d, cx| {
                d.machine = Some(m);
                d.system = system;
                d.detecting = false;
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    /// Which models are on disk (re-hashing one whose record does not
    /// match) and the free space. `again`: even when known.
    pub fn refresh_models(&mut self, again: bool, cx: &mut Context<Self>) {
        if self.checking || (!again && !self.on_disk.is_empty()) {
            return;
        }
        let Some(dir) = Self::models_dir() else {
            return;
        };
        self.checking = true;
        let task = cx.background_spawn(async move {
            let states: HashMap<ModelId, ModelState> = ModelId::ALL
                .into_iter()
                .map(|m| (m, download::check_model(&dir, m.file().into())))
                .collect();
            // The folder may not exist yet: its disk is the data folder's.
            let probe = if dir.exists() {
                dir.clone()
            } else {
                dir.parent().map(PathBuf::from).unwrap_or(dir)
            };
            (states, download::free_space(&probe).ok())
        });
        cx.spawn(async move |d, cx| {
            let (states, free) = task.await;
            let _ = d.update(cx, |d, cx| {
                d.on_disk = states;
                d.free = free;
                d.checking = false;
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    /// The input devices, for the Microphone row.
    pub fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        if self.devices.is_some() {
            return;
        }
        self.devices = Some(Vec::new());
        #[cfg(debug_assertions)]
        if self.dev.active() {
            // Captures never list this machine's devices.
            return;
        }
        let task = cx.background_spawn(async { capture::devices() });
        cx.spawn(async move |d, cx| {
            let list = task.await;
            let _ = d.update(cx, |d, cx| {
                d.devices = Some(list);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    // ---- choices ----

    pub fn set_engine(&mut self, choice: EngineChoice, cx: &mut Context<Self>) {
        if choice == EngineChoice::Off {
            self.request_stop(StopReason::Asked, cx);
        }
        settings::change(cx, |p| p.voice.engine = Some(choice));
        cx.refresh_windows();
    }

    pub fn use_model(&mut self, model: ModelId, cx: &mut Context<Self>) {
        settings::change(cx, |p| {
            p.voice.model = Some(model);
            p.voice.engine = Some(EngineChoice::Whisper);
        });
        cx.refresh_windows();
    }

    pub fn set_language(&mut self, language: Lang, cx: &mut Context<Self>) {
        settings::change(cx, |p| p.voice.language = language);
        cx.refresh_windows();
    }

    pub fn set_gpu(&mut self, gpu: bool, cx: &mut Context<Self>) {
        settings::change(cx, |p| p.voice.gpu = gpu);
        cx.refresh_windows();
    }

    /// The chosen microphone (`None`: the system default), kept in the
    /// database's local values: a device name can carry a person's name.
    pub fn set_microphone(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.microphone = id.clone();
        if let Some(db) = &self.db {
            db.run(cx, move |store| {
                store.set_local_value(MICROPHONE_KEY, id.as_deref())
            })
            .detach();
        }
        // A preview on the old device starts again on the new one.
        if self.run.as_ref().is_some_and(|r| r.preview) {
            self.stop_preview();
        }
        cx.refresh_windows();
    }

    // ---- download and delete ----

    /// Downloads `model` after a click that named its size (ADR 0014, 4).
    /// `choose`: it becomes the model in use (the first-use dialog).
    pub fn start_download(&mut self, model: ModelId, choose: bool, cx: &mut Context<Self>) {
        if self.download.is_some() {
            return;
        }
        // The model a take is using is not replaced under it.
        let in_use =
            matches!(&self.engine_for, Some(EngineFor::Whisper { model: m, .. }) if *m == model);
        if in_use && self.running() {
            self.refusal = Some(crate::i18n::Key::VoiceModelBusy);
            cx.refresh_windows();
            return;
        }
        let Some(dir) = Self::models_dir() else {
            return;
        };
        if choose {
            self.use_model(model, cx);
        }
        self.finished = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(Progress::Bytes {
            done: 0,
            total: model.file().size,
        }));
        // Downloading over the model in use: the worker lets go first.
        let release = self.take_engine_for(model);
        let (c, p) = (Arc::clone(&cancel), Arc::clone(&progress));
        let url = Self::download_url(model);
        let policy = Self::download_policy();
        let work = cx.background_spawn(async move {
            if let Some(mut controller) = release
                && !controller.release_engine(Duration::from_secs(10))
            {
                return Err(DownloadError::Busy);
            }
            let mut report = |progress: Progress| {
                *p.lock().unwrap_or_else(|e| e.into_inner()) = progress;
            };
            download::download(download::Request {
                url,
                expected: model.file().into(),
                dir: &dir,
                policy,
                free_space: &download::free_space,
                cancel: &c,
                progress: &mut report,
            })
            .map(|_| ())
        });
        let task = cx.spawn(async move |d, cx| {
            let result = work.await;
            let _ = d.update(cx, |d, cx| d.download_done(model, result, cx));
        });
        crate::log::line(&format!("voice: download of {} started", model.key()));
        self.download = Some(Download {
            model,
            cancel,
            progress,
            _task: task,
        });
        self.ensure_polling(cx);
        cx.notify();
    }

    fn download_url(model: ModelId) -> String {
        #[cfg(debug_assertions)]
        if let Some(base) = &dev::Dev::from_env().url {
            return format!("{base}{}", model.file().file);
        }
        model.file().url()
    }

    fn download_policy() -> Policy {
        #[cfg(debug_assertions)]
        if dev::Dev::from_env().url.is_some() {
            return Policy {
                start: download::Schemes::HttpsOrLoopback,
                redirects: download::Schemes::HttpsOrLoopback,
            };
        }
        Policy::RELEASE
    }

    fn download_done(
        &mut self,
        model: ModelId,
        result: Result<(), DownloadError>,
        cx: &mut Context<Self>,
    ) {
        self.download = None;
        match result {
            Ok(()) => {
                crate::log::line(&format!("voice: download of {} done", model.key()));
                self.finished = Some(Finished::Ready(model));
            }
            Err(DownloadError::Cancelled) => {
                crate::log::line("voice: download cancelled");
                self.finished = None;
            }
            Err(e) => {
                crate::log::line(&format!("voice: download failed ({e})"));
                self.finished = Some(Finished::Failed(model, e));
            }
        }
        self.refresh_models(true, cx);
        cx.notify();
        cx.refresh_windows();
    }

    /// "Cancel download": the partial file is deleted.
    pub fn cancel_download(&mut self) {
        if let Some(d) = &self.download {
            d.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// "Delete model", confirmed. Refused while a take runs.
    pub fn delete_model(&mut self, model: ModelId, cx: &mut Context<Self>) {
        if self.running() {
            self.refusal = Some(crate::i18n::Key::VoiceModelBusy);
            cx.refresh_windows();
            return;
        }
        self.refusal = None;
        let Some(dir) = Self::models_dir() else {
            return;
        };
        let release = self.take_engine_for(model);
        let work = cx.background_spawn(async move {
            if let Some(mut controller) = release
                && !controller.release_engine(Duration::from_secs(10))
            {
                return false;
            }
            download::delete_model(&dir, model.file().file).is_ok()
        });
        cx.spawn(async move |d, cx| {
            let deleted = work.await;
            crate::log::line(&format!(
                "voice: model {} {}",
                model.key(),
                if deleted { "deleted" } else { "not deleted" }
            ));
            let _ = d.update(cx, |d, cx| d.refresh_models(true, cx));
        })
        .detach();
    }

    /// The controller, when its engine uses `model`: taken out so its
    /// worker can be ended (ADR 0014, amendment: `release_engine` before a
    /// model file is replaced or deleted).
    fn take_engine_for(&mut self, model: ModelId) -> Option<Controller> {
        match &self.engine_for {
            Some(EngineFor::Whisper { model: m, .. }) if *m == model => {
                self.engine_for = None;
                self.controller.take()
            }
            _ => None,
        }
    }

    // ---- takes ----

    fn wanted_engine(&self, cx: &App) -> Option<EngineFor> {
        #[cfg(debug_assertions)]
        if self.dev.fake.is_some() {
            return Some(EngineFor::Fake);
        }
        let prefs = Self::prefs(cx);
        if prefs.engine == Some(EngineChoice::MacOs) {
            return Some(EngineFor::MacOs {
                french_interface: settings::locale(cx) == crate::i18n::Locale::Fr,
            });
        }
        let model = prefs.model?;
        let metal = self.machine.is_some_and(|m| m.metal);
        Some(EngineFor::Whisper {
            model,
            gpu: prefs.gpu && metal,
        })
    }

    fn make_engine(&self, wanted: &EngineFor) -> Option<Box<dyn Engine>> {
        match wanted {
            EngineFor::Whisper { model, gpu } => {
                let exe = std::env::current_exe().ok()?;
                let path = Self::models_dir()?.join(model.file().file);
                // Silero VAD comes with the package: next to the executable,
                // or in the bundle's Resources on macOS (Contents/MacOS holds
                // code only).
                let vad = exe.parent().and_then(|dir| {
                    let resources = dir.join("../Resources");
                    [dir, resources.as_path()]
                        .into_iter()
                        .map(|d| d.join(models::SILERO_VAD.file))
                        .find(|p| p.is_file())
                });
                let cores = self.machine.map_or(4, |m| m.physical_cores);
                Some(Box::new(WorkerClient::new(
                    exe,
                    EngineSpec::Whisper {
                        model: path,
                        vad,
                        threads: hardware::decode_threads(cores),
                        gpu: *gpu,
                    },
                )))
            }
            #[cfg(target_os = "macos")]
            EngineFor::MacOs { french_interface } => {
                Some(Box::new(macos::MacRecognizer::new(*french_interface)))
            }
            #[cfg(not(target_os = "macos"))]
            EngineFor::MacOs { .. } => None,
            #[cfg(debug_assertions)]
            EngineFor::Fake => Some(Box::new(self.dev.fake_engine())),
        }
    }

    /// The controller for the current settings, made again when they
    /// changed (the old one ends its worker off the UI thread).
    fn controller(&mut self, cx: &App) -> Option<&mut Controller> {
        let wanted = self.wanted_engine(cx)?;
        if self.engine_for.as_ref() != Some(&wanted) || self.controller.is_none() {
            let engine = self.make_engine(&wanted)?;
            if let Some(old) = self.controller.replace(Controller::new(engine)) {
                std::thread::spawn(move || drop(old));
            }
            self.engine_for = Some(wanted);
        }
        self.controller.as_mut()
    }

    /// The microphone (or the key) was pressed in `editor`.
    pub fn press(&mut self, editor: EditorId, window: &mut Window, cx: &mut Context<Self>) {
        match self.phase(editor, cx) {
            Phase::Off | Phase::Writing { .. } => {}
            Phase::NotSetUp => window.dispatch_action(Box::new(OpenVoiceSetup), cx),
            Phase::Downloading(_) => window.dispatch_action(Box::new(OpenVoiceSettings), cx),
            Phase::Starting { .. } | Phase::Listening { .. } => {
                self.request_stop(StopReason::Asked, cx)
            }
            Phase::Idle => self.start(editor, cx),
        }
    }

    fn running_in(&self, editor: EditorId, cx: &App) -> bool {
        matches!(
            self.phase(editor, cx),
            Phase::Starting { .. } | Phase::Listening { .. }
        )
    }

    fn key_down(&mut self, editor: EditorId, window: &mut Window, cx: &mut Context<Self>) {
        let running = self.running_in(editor, cx);
        self.gesture_editor = Some(editor);
        match self.gestures.down(Instant::now(), running) {
            Gesture::Nothing => {}
            Gesture::Stop => self.request_stop(StopReason::Asked, cx),
            Gesture::Start => self.press(editor, window, cx),
        }
    }

    fn gesture_up(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.gesture_editor else {
            return;
        };
        let running = self.running_in(editor, cx);
        if self.gestures.up(Instant::now(), running) == Gesture::Stop {
            self.request_stop(StopReason::Asked, cx);
        }
    }

    /// The mouse button went down on the microphone of `editor`.
    pub fn mouse_down(&mut self, editor: EditorId, window: &mut Window, cx: &mut Context<Self>) {
        // Focus stays in (or returns to) the editor.
        if let Some(state) = self.editors.get(&editor).and_then(|s| s.state.upgrade()) {
            state.update(cx, |s, cx| s.focus(window, cx));
        }
        self.key_down(editor, window, cx);
    }

    pub fn mouse_up(&mut self, cx: &mut Context<Self>) {
        self.gesture_up(cx);
    }

    /// Palette "Dictate": the target editor gets the focus and a take.
    pub fn dictate_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.target() else { return };
        if let Some(state) = self.editors.get(&editor).and_then(|s| s.state.upgrade()) {
            state.update(cx, |s, cx| s.focus(window, cx));
        }
        if !self.running_in(editor, cx) {
            self.press(editor, window, cx);
        }
    }

    fn start(&mut self, editor: EditorId, cx: &mut Context<Self>) {
        let Some(state) = self.editors.get(&editor).and_then(|s| s.state.upgrade()) else {
            return;
        };
        let caret = state.read(cx).cursor();
        self.capture_errors.remove(&editor);
        self.writing_since.remove(&editor);
        let now = Instant::now();
        if let Some(run) = &self.run {
            if run.preview {
                self.stop_preview();
            } else {
                // The take elsewhere ends first; this one waits for its
                // last phrase.
                self.request_stop(StopReason::Asked, cx);
                self.pending = Some(Pending {
                    editor,
                    caret,
                    at: now,
                });
                self.ensure_polling(cx);
                cx.refresh_windows();
                return;
            }
        }
        self.begin(editor, caret, now, cx);
    }

    fn begin(&mut self, editor: EditorId, caret: usize, at: Instant, cx: &mut Context<Self>) {
        let lang = Self::prefs(cx).language;
        let Some(controller) = self.controller(cx) else {
            return;
        };
        controller.start(editor, caret, lang);
        let (tx, events) = mpsc::channel();
        let handle = self.open_capture(tx);
        crate::log::line("voice: take started");
        self.run = Some(Run {
            editor,
            handle,
            events,
            asked_at: at,
            started_at: None,
            stop: None,
            discarding: false,
            error: None,
            preview: false,
        });
        self.ensure_polling(cx);
        cx.refresh_windows();
    }

    fn open_capture(&self, tx: mpsc::Sender<capture::Event>) -> capture::Handle {
        #[cfg(debug_assertions)]
        if self.dev.active() {
            // Never a device in debug captures: generated tone bursts.
            let (rate, channels, signal) = dev::signal();
            return capture::start_synthetic(rate, channels, signal, tx);
        }
        capture::start(self.microphone.clone(), tx)
    }

    /// Stops the take in progress (its last phrase is still written).
    pub fn request_stop(&mut self, reason: StopReason, cx: &mut Context<Self>) {
        if let Some(run) = self.run.as_mut().filter(|r| !r.preview && r.stop.is_none()) {
            run.stop = Some((reason, Instant::now()));
            run.handle.stop();
        }
        cx.refresh_windows();
    }

    /// `Esc` in `editor`: true when it stopped a take (the key is then
    /// not the editor's).
    fn escape(&mut self, editor: EditorId, cx: &mut Context<Self>) -> bool {
        let pending = self.pending.as_ref().is_some_and(|p| p.editor == editor);
        if pending {
            self.pending = None;
        }
        let writing = self
            .controller
            .as_ref()
            .is_some_and(|c| c.state(editor) == TakeState::Writing);
        let run = self
            .run
            .as_mut()
            .filter(|r| r.editor == editor && !r.preview && !r.discarding);
        let had_run = run.is_some();
        if let Some(run) = run {
            run.discarding = true;
            run.handle.discard();
        }
        if !(had_run || writing || pending) {
            return false;
        }
        if let Some(c) = &mut self.controller {
            c.escape(editor);
        }
        if self.save_after == Some(editor) {
            self.save_after = None;
        }
        crate::log::line("voice: take dropped");
        cx.refresh_windows();
        true
    }

    /// `Mod+Enter` in `editor`: a take running or writing there stops,
    /// and the editor saves once its last phrase is written.
    fn save_after_take(&mut self, editor: EditorId, cx: &mut Context<Self>) -> bool {
        let busy = matches!(
            self.phase(editor, cx),
            Phase::Starting { .. } | Phase::Listening { .. } | Phase::Writing { .. }
        );
        if !busy {
            return false;
        }
        self.request_stop(StopReason::Asked, cx);
        self.save_after = Some(editor);
        true
    }

    fn blurred(&mut self, editor: EditorId, cx: &mut Context<Self>) {
        if self
            .run
            .as_ref()
            .is_some_and(|r| r.editor == editor && !r.preview)
        {
            self.request_stop(StopReason::FocusLost, cx);
        }
        if self.pending.as_ref().is_some_and(|p| p.editor == editor) {
            self.pending = None;
        }
    }

    fn edited(&mut self, editor: EditorId, event: &InputEditsEvent) {
        let Some(slot) = self.editors.get_mut(&editor) else {
            return;
        };
        for (edit, _) in event.edits() {
            let at = edit.start_byte;
            let removed = edit.old_end_byte.saturating_sub(at);
            let inserted = edit.new_end_byte.saturating_sub(at);
            if removed == 0 && slot.own.front() == Some(&(at, inserted)) {
                slot.own.pop_front();
                continue;
            }
            if let Some(c) = &mut self.controller {
                c.edited(editor, at, removed, inserted);
            }
        }
    }

    // ---- the Settings level bar ----

    /// The person is at the Microphone row (pointer over it, or its
    /// dropdown focused or open): its level bar runs on the chosen device
    /// while they stay there, and stops within a second after.
    pub fn preview_shown(&mut self, cx: &mut Context<Self>) {
        self.preview_seen = Some(Instant::now());
        let engine = Self::prefs(cx).engine;
        let listens = matches!(engine, Some(EngineChoice::Whisper | EngineChoice::MacOs));
        if self.run.is_some() || self.pending.is_some() || !listens {
            return;
        }
        let (tx, events) = mpsc::channel();
        let handle = self.open_capture(tx);
        self.run = Some(Run {
            editor: 0,
            handle,
            events,
            asked_at: Instant::now(),
            started_at: None,
            stop: None,
            discarding: false,
            error: None,
            preview: true,
        });
        self.ensure_polling(cx);
    }

    fn stop_preview(&mut self) {
        if let Some(run) = self.run.take_if(|r| r.preview) {
            run.handle.discard();
        }
        self.level = acta_core::voice::level::FLOOR_DBFS;
    }

    // ---- the loop ----

    fn ensure_polling(&mut self, cx: &mut Context<Self>) {
        if self.poll.is_some() {
            return;
        }
        self.poll = Some(cx.spawn(async move |d, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let more = d.update(cx, |d, cx| d.tick(cx)).unwrap_or(false);
                if !more {
                    let _ = d.update(cx, |d, _| d.poll = None);
                    break;
                }
            }
        }));
    }

    /// One look at the take; `false` when nothing is left to watch.
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        self.drop_closed_editors();
        self.drain_capture(cx);
        // A preview no longer on screen stops.
        if self.run.as_ref().is_some_and(|r| r.preview)
            && self
                .preview_seen
                .is_none_or(|at| at.elapsed() > Duration::from_secs(1))
        {
            self.stop_preview();
        }
        // The engine failed under a running take: the capture stops.
        if let (Some(run), Some(c)) = (self.run.as_mut(), self.controller.as_ref())
            && !run.preview
            && !run.discarding
            && run.stop.is_none()
            && matches!(c.state(run.editor), TakeState::Error(_))
        {
            run.discarding = true;
            run.handle.discard();
        }
        self.apply_outputs(cx);
        cx.refresh_windows();
        self.busy()
    }

    fn busy(&self) -> bool {
        self.run.is_some()
            || self.pending.is_some()
            || self.download.is_some()
            || self.controller.as_ref().is_some_and(|c| {
                self.editors.keys().any(|id| {
                    matches!(
                        c.state(*id),
                        TakeState::Starting | TakeState::Listening | TakeState::Writing
                    )
                })
            })
    }

    fn drop_closed_editors(&mut self) {
        let closed: Vec<EditorId> = self
            .editors
            .iter()
            .filter(|(_, s)| s.state.upgrade().is_none())
            .map(|(id, _)| *id)
            .collect();
        for id in closed {
            self.editors.remove(&id);
            self.by_entity.retain(|_, e| *e != id);
            if let Some(run) = self.run.as_mut().filter(|r| r.editor == id && !r.preview) {
                run.discarding = true;
                run.handle.discard();
            }
            if self.pending.as_ref().is_some_and(|p| p.editor == id) {
                self.pending = None;
            }
            if let Some(c) = &mut self.controller {
                c.closed(id);
            }
            self.capture_errors.remove(&id);
            self.writing_since.remove(&id);
            if self.last_focused == Some(id) {
                self.last_focused = None;
            }
        }
    }

    fn drain_capture(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let mut ended = false;
        while let Ok(event) = run.events.try_recv() {
            match event {
                capture::Event::Started => {
                    run.started_at = Some(Instant::now());
                    if !run.preview
                        && let Some(c) = &mut self.controller
                    {
                        c.capture_started();
                    }
                }
                capture::Event::Level(l) => self.level = l,
                capture::Event::Phrase(p) => {
                    if !run.preview
                        && !run.discarding
                        && let Some(c) = &mut self.controller
                    {
                        c.phrase(p);
                    }
                }
                capture::Event::SilenceStop => {
                    run.stop
                        .get_or_insert((StopReason::Silence, Instant::now()));
                }
                capture::Event::LimitStop => {
                    run.stop.get_or_insert((StopReason::Limit, Instant::now()));
                }
                capture::Event::Error(e) => run.error = Some(e),
                capture::Event::Ended => {
                    ended = true;
                    break;
                }
            }
        }
        if !ended {
            return;
        }
        let Some(run) = self.run.take() else { return };
        self.level = acta_core::voice::level::FLOOR_DBFS;
        if run.preview {
            return;
        }
        let editor = run.editor;
        if !run.discarding
            && let Some(c) = &mut self.controller
        {
            if run.started_at.is_none() {
                // The microphone never started: nothing to write, nothing
                // to save after.
                c.escape(editor);
                if self.save_after == Some(editor) {
                    self.save_after = None;
                }
            } else {
                let reason = run.stop.map_or(StopReason::Asked, |(r, _)| r);
                c.stop(reason);
            }
        }
        if let Some(e) = run.error {
            crate::log::line(&format!("voice: microphone error ({e:?})"));
            self.capture_errors.insert(editor, e);
        }
        self.writing_since
            .insert(editor, run.stop.map_or_else(Instant::now, |(_, at)| at));
        if let Some(p) = self.pending.take() {
            self.begin(p.editor, p.caret, p.at, cx);
        }
    }

    fn apply_outputs(&mut self, cx: &mut Context<Self>) {
        let Some(controller) = self.controller.as_mut() else {
            return;
        };
        let snapshot = Snapshot::of(&self.editors, controller, cx);
        let outputs = controller.poll(&snapshot);
        for output in outputs {
            match output {
                Output::Insert {
                    editor, insertion, ..
                } => {
                    let Some(slot) = self.editors.get_mut(&editor) else {
                        continue;
                    };
                    let Some(state) = slot.state.upgrade() else {
                        continue;
                    };
                    slot.own.push_back((insertion.at, insertion.text.len()));
                    let _ = slot.window.update(cx, |_, window, cx| {
                        state.update(cx, |s, cx| apply(s, &insertion, window, cx));
                    });
                }
                Output::Done { editor, .. } => {
                    if self.save_after == Some(editor) {
                        self.save_after = None;
                        self.save(editor, cx);
                    }
                }
            }
        }
    }

    /// The take stopped by `Mod+Enter` is written: the editor saves, as
    /// if the key had just been pressed, when it still has the focus.
    fn save(&mut self, editor: EditorId, cx: &mut Context<Self>) {
        let Some(slot) = self.editors.get(&editor) else {
            return;
        };
        let Some(state) = slot.state.upgrade() else {
            return;
        };
        let key = if cfg!(target_os = "macos") {
            "cmd-enter"
        } else {
            "ctrl-enter"
        };
        let _ = slot.window.update(cx, |_, window, cx| {
            if state.read(cx).focus_handle(cx).is_focused(window)
                && let Ok(k) = gpui_kit::Keystroke::parse(key)
            {
                window.dispatch_keystroke(k, cx);
            }
        });
    }
}

/// Writes a phrase into the editor at the anchor, leaving the person's
/// selection where it was (shifted past the new text), or the caret after
/// it when it was at the anchor.
fn apply(
    s: &mut TextareaState,
    ins: &acta_core::voice::insert::Insertion,
    window: &mut Window,
    cx: &mut Context<TextareaState>,
) {
    let len = ins.text.len();
    let selection = s.selected_range();
    let shift = |p: usize| if p >= ins.at { p + len } else { p };
    s.set_selected_range(ins.at..ins.at, cx);
    s.insert(ins.text.clone(), window, cx);
    match ins.caret {
        Some(c) => s.set_selected_range(c..c, cx),
        None => s.set_selected_range(shift(selection.start)..shift(selection.end), cx),
    }
}

/// The editors' texts and carets as the controller reads them in a poll.
struct Snapshot(HashMap<EditorId, (String, usize)>);

impl Snapshot {
    fn of(editors: &HashMap<EditorId, Slot>, c: &Controller, cx: &App) -> Self {
        Snapshot(
            editors
                .iter()
                .filter(|(id, _)| c.state(**id) != TakeState::Idle)
                .filter_map(|(id, slot)| {
                    let state = slot.state.upgrade()?;
                    let s = state.read(cx);
                    Some((*id, (s.value().to_string(), s.cursor())))
                })
                .collect(),
        )
    }
}

impl Editors for Snapshot {
    fn text_and_caret(&self, editor: EditorId) -> Option<(&str, usize)> {
        self.0.get(&editor).map(|(t, c)| (t.as_str(), *c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_macos_recogniser_follows_the_take_language() {
        let fr_only = OnDevice {
            en: false,
            fr: true,
        };
        assert!(fr_only.any());
        assert!(fr_only.supports(Lang::French, false));
        assert!(!fr_only.supports(Lang::English, true));
        // "Detected" is the interface's language.
        assert!(fr_only.supports(Lang::Detect, true));
        assert!(!fr_only.supports(Lang::Detect, false));
        assert!(!OnDevice::default().any());
    }

    #[test]
    fn the_macos_choice_is_kept_only_on_macos() {
        let prefs = VoicePrefs::from_stored(Some("macos"), None, None, false);
        if cfg!(target_os = "macos") {
            assert_eq!(prefs.engine, Some(EngineChoice::MacOs));
            assert_eq!(prefs.engine_key(), Some("macos"));
        } else {
            assert_eq!(prefs.engine, None);
        }
    }
}
