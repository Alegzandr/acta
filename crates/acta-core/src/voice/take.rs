//! Takes (ADR 0014, 6; components-voice.md 5): one at a time in the whole
//! app, phrases written at the anchor in order as the engine returns them,
//! never rewritten. The [`Controller`] owns the engine on a thread of its
//! own and is driven by the app: capture hands it phrases, the UI hands it
//! keys, focus changes and edits, and [`Controller::poll`] returns the
//! insertions to apply.
//!
//! States per editor: idle, starting, listening, writing, error. A take
//! stopped while phrases are still with the engine is *writing* until they
//! come back; starting a take in a second editor stops the first, whose
//! last phrases still go to the first editor.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use super::buffer::AudioBuf;
use super::engine::{Engine, EngineError, Lang};
use super::insert::{Anchor, Insertion};

/// The app's id for an editor (opaque here).
pub type EditorId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeState {
    Idle,
    /// Microphone opening, model loading.
    Starting,
    Listening,
    /// Stopped; the last phrases are being turned into text.
    Writing,
    Error(EngineError),
}

/// Why a take stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The button, the key, its release, or `Mod+Enter`.
    Asked,
    /// Focus left the editor, or it closed.
    FocusLost,
    /// 60 s without speech.
    Silence,
    /// 10 minutes.
    Limit,
}

/// A line after a take, in `text.tertiary` until the next take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// `voice.stopped_silence`
    StoppedSilence,
    /// `voice.stopped_limit`
    StoppedLimit,
    /// `voice.nothing_heard`
    NothingHeard,
}

/// What the app applies after [`Controller::poll`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    /// Insert into `editor`; every insertion of one `take` is one undo
    /// step. The app must not report this edit back through `edited`: the
    /// anchor already moved.
    Insert {
        editor: EditorId,
        take: u64,
        insertion: Insertion,
    },
    /// The take in `editor` is over and its last text is written (the
    /// moment `Mod+Enter` saves).
    Done { editor: EditorId, take: u64 },
}

/// The editors' current text and caret, read when a phrase comes back.
pub trait Editors {
    /// `None` when the editor is gone: its text is dropped.
    fn text_and_caret(&self, editor: EditorId) -> Option<(&str, usize)>;
}

/// The editors as the outputs of the current poll leave them.
struct Applied<'a> {
    editors: &'a dyn Editors,
    changed: Vec<(EditorId, String, usize)>,
}

impl Applied<'_> {
    fn text_and_caret(&self, editor: EditorId) -> Option<(String, usize)> {
        match self.changed.iter().find(|(e, _, _)| *e == editor) {
            Some((_, text, caret)) => Some((text.clone(), *caret)),
            None => self
                .editors
                .text_and_caret(editor)
                .map(|(t, c)| (t.to_string(), c)),
        }
    }

    fn apply(&mut self, editor: EditorId, mut text: String, caret: usize, ins: &Insertion) {
        text.insert_str(ins.at, &ins.text);
        let caret = match ins.caret {
            Some(c) => c,
            None if caret >= ins.at => caret + ins.text.len(),
            None => caret,
        };
        self.changed.retain(|(e, _, _)| *e != editor);
        self.changed.push((editor, text, caret));
    }
}

struct Take {
    id: u64,
    editor: EditorId,
    anchor: Anchor,
    /// Phrases sent to the engine, not yet back.
    pending: VecDeque<u64>,
    next_seq: u64,
    engine_started: bool,
    capture_started: bool,
    stopped: bool,
    wrote: bool,
    reason: Option<StopReason>,
}

enum Job {
    Start {
        take: u64,
        lang: Lang,
    },
    Feed {
        take: u64,
        seq: u64,
        phrase: AudioBuf,
    },
    Stop,
    Release(Sender<()>),
}

enum Outcome {
    Started {
        take: u64,
        result: Result<(), EngineError>,
    },
    Text {
        take: u64,
        seq: u64,
        result: Result<String, EngineError>,
    },
}

pub struct Controller {
    jobs: Option<Sender<Job>>,
    outcomes: Receiver<Outcome>,
    /// Takes below this id are dropped: the engine thread skips their
    /// phrases (and wipes them) instead of decoding.
    dropped_below: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
    active: Option<Take>,
    /// Stopped takes still writing.
    finishing: Vec<Take>,
    /// Per-editor state that outlives a take: an error or a notice.
    errors: Vec<(EditorId, EngineError)>,
    notices: Vec<(EditorId, Notice)>,
    next_take: u64,
}

impl Controller {
    pub fn new(engine: Box<dyn Engine>) -> Self {
        let (jobs, job_rx) = mpsc::channel();
        let (out_tx, outcomes) = mpsc::channel();
        let dropped_below = Arc::new(AtomicU64::new(0));
        let dropped = Arc::clone(&dropped_below);
        let thread = std::thread::Builder::new()
            .name("acta-voice-engine".into())
            .spawn(move || run_engine(engine, job_rx, out_tx, dropped))
            .ok();
        Controller {
            jobs: Some(jobs),
            outcomes,
            dropped_below,
            thread,
            active: None,
            finishing: Vec::new(),
            errors: Vec::new(),
            notices: Vec::new(),
            next_take: 1,
        }
    }

    /// The state the microphone of `editor` shows.
    pub fn state(&self, editor: EditorId) -> TakeState {
        if let Some(t) = self.active.as_ref().filter(|t| t.editor == editor) {
            return if t.engine_started && t.capture_started {
                TakeState::Listening
            } else {
                TakeState::Starting
            };
        }
        if self.finishing.iter().any(|t| t.editor == editor) {
            return TakeState::Writing;
        }
        match self.errors.iter().find(|(e, _)| *e == editor) {
            Some((_, err)) => TakeState::Error(*err),
            None => TakeState::Idle,
        }
    }

    pub fn notice(&self, editor: EditorId) -> Option<Notice> {
        self.notices
            .iter()
            .find(|(e, _)| *e == editor)
            .map(|(_, n)| *n)
    }

    /// The editor of the running take, if any.
    pub fn active_editor(&self) -> Option<EditorId> {
        self.active.as_ref().map(|t| t.editor)
    }

    /// Starts a take in `editor` at `caret`; a take running elsewhere is
    /// stopped first (its last phrases still go to its editor). Returns the
    /// take id.
    pub fn start(&mut self, editor: EditorId, caret: usize, lang: Lang) -> u64 {
        if self.active.is_some() {
            self.stop(StopReason::Asked);
        }
        let id = self.next_take;
        self.next_take += 1;
        self.errors.retain(|(e, _)| *e != editor);
        self.notices.retain(|(e, _)| *e != editor);
        self.active = Some(Take {
            id,
            editor,
            anchor: Anchor::new(caret),
            pending: VecDeque::new(),
            next_seq: 0,
            engine_started: false,
            capture_started: false,
            stopped: false,
            wrote: false,
            reason: None,
        });
        self.send(Job::Start { take: id, lang });
        id
    }

    /// The microphone is delivering audio.
    pub fn capture_started(&mut self) {
        if let Some(t) = &mut self.active {
            t.capture_started = true;
        }
    }

    /// A phrase from the segmenter of the running take.
    pub fn phrase(&mut self, phrase: AudioBuf) {
        let Some(t) = &mut self.active else {
            return;
        };
        let seq = t.next_seq;
        t.next_seq += 1;
        t.pending.push_back(seq);
        let take = t.id;
        self.send(Job::Feed { take, seq, phrase });
    }

    /// Stops the running take; it writes its pending phrases, then is done.
    pub fn stop(&mut self, reason: StopReason) {
        let Some(mut t) = self.active.take() else {
            return;
        };
        t.stopped = true;
        t.reason = Some(reason);
        self.send(Job::Stop);
        self.finishing.push(t);
    }

    /// `Esc`: the running take (or the one writing in `editor`) stops and
    /// what is not written yet is thrown away. Text already written stays.
    pub fn escape(&mut self, editor: EditorId) {
        let mut dropped = Vec::new();
        if let Some(t) = self.active.take_if(|t| t.editor == editor) {
            self.send(Job::Stop);
            dropped.push(t);
        }
        let (gone, kept): (Vec<Take>, Vec<Take>) = std::mem::take(&mut self.finishing)
            .into_iter()
            .partition(|t| t.editor == editor);
        self.finishing = kept;
        dropped.extend(gone);
        if let Some(max) = dropped.iter().map(|t| t.id).max() {
            // Takes are numbered in order, and every older take in this
            // editor is over; older takes elsewhere are finishing and must
            // keep their phrases, so only raise the bar when none is left.
            let oldest_elsewhere = self.finishing.iter().map(|t| t.id).min();
            let bar = match oldest_elsewhere {
                Some(o) if o <= max => o,
                _ => max + 1,
            };
            self.dropped_below.fetch_max(bar, Ordering::SeqCst);
        }
    }

    /// Before a model file is downloaded over, replaced or deleted: every
    /// take stops (unwritten phrases are dropped, as with `Esc`) and the
    /// engine lets go of the model (the worker process exits). `false` when
    /// the engine did not confirm within `limit`; the caller must then not
    /// touch the file.
    pub fn release_engine(&mut self, limit: std::time::Duration) -> bool {
        let editors: Vec<EditorId> = self
            .active
            .iter()
            .chain(self.finishing.iter())
            .map(|t| t.editor)
            .collect();
        for editor in editors {
            self.escape(editor);
        }
        let (done, confirmed) = mpsc::channel();
        self.send(Job::Release(done));
        confirmed.recv_timeout(limit).is_ok()
    }

    /// The editor closed: its take stops; text that comes back is dropped
    /// (the app writes the draft it holds). Its error and notice go.
    pub fn closed(&mut self, editor: EditorId) {
        self.escape(editor);
        self.errors.retain(|(e, _)| *e != editor);
        self.notices.retain(|(e, _)| *e != editor);
    }

    /// The text of `editor` changed by the person (not by an
    /// [`Output::Insert`]): `removed` bytes at `at` became `inserted`.
    pub fn edited(&mut self, editor: EditorId, at: usize, removed: usize, inserted: usize) {
        for t in self.active.iter_mut().chain(self.finishing.iter_mut()) {
            if t.editor == editor {
                t.anchor.edited(at, removed, inserted);
            }
        }
    }

    /// Applies what the engine returned. Insertions are computed against
    /// the editors' current text and caret.
    ///
    /// Several phrases can come back in one poll: each is placed against
    /// the text as the earlier ones of this poll leave it, so the app
    /// applies the outputs in order.
    pub fn poll(&mut self, editors: &dyn Editors) -> Vec<Output> {
        let mut out = Vec::new();
        let mut view = Applied {
            editors,
            changed: Vec::new(),
        };
        while let Ok(outcome) = self.outcomes.try_recv() {
            match outcome {
                Outcome::Started { take, result } => {
                    if let Some(t) = self.active.as_mut().filter(|t| t.id == take) {
                        match result {
                            Ok(()) => t.engine_started = true,
                            Err(e) => {
                                let editor = t.editor;
                                self.active = None;
                                self.send(Job::Stop);
                                self.fail(editor, e);
                            }
                        }
                    }
                }
                Outcome::Text { take, seq, result } => {
                    self.text(take, seq, result, &mut view, &mut out);
                }
            }
        }
        // Stopped takes with nothing pending are done.
        let mut i = 0;
        while i < self.finishing.len() {
            if self.finishing[i].pending.is_empty() {
                let t = self.finishing.remove(i);
                self.done(t, &mut out);
            } else {
                i += 1;
            }
        }
        out
    }

    fn take_mut(&mut self, take: u64) -> Option<&mut Take> {
        self.active
            .iter_mut()
            .chain(self.finishing.iter_mut())
            .find(|t| t.id == take)
    }

    fn text(
        &mut self,
        take: u64,
        seq: u64,
        result: Result<String, EngineError>,
        view: &mut Applied<'_>,
        out: &mut Vec<Output>,
    ) {
        let Some(t) = self.take_mut(take) else {
            return;
        };
        // The engine answers in order; anything else is a stale answer.
        if t.pending.front() != Some(&seq) {
            return;
        }
        t.pending.pop_front();
        let editor = t.editor;
        match result {
            Ok(text) => {
                if let Some((current, caret)) = view.text_and_caret(editor)
                    && let Some(insertion) = t.anchor.insert(&current, &text, caret)
                {
                    t.wrote = true;
                    view.apply(editor, current, caret, &insertion);
                    out.push(Output::Insert {
                        editor,
                        take,
                        insertion,
                    });
                }
            }
            // One phrase lost; the take goes on.
            Err(EngineError::Failed) => {}
            Err(e) => {
                // The engine is gone: written text stays, the rest is lost.
                let was_active = self.active.as_ref().is_some_and(|t| t.id == take);
                if was_active {
                    self.active = None;
                    self.send(Job::Stop);
                } else {
                    self.finishing.retain(|t| t.id != take);
                }
                self.dropped_below.fetch_max(take + 1, Ordering::SeqCst);
                self.fail(editor, e);
                out.push(Output::Done { editor, take });
            }
        }
    }

    fn fail(&mut self, editor: EditorId, e: EngineError) {
        self.errors.retain(|(ed, _)| *ed != editor);
        self.errors.push((editor, e));
    }

    fn done(&mut self, t: Take, out: &mut Vec<Output>) {
        let notice = match t.reason {
            Some(StopReason::Silence) => Some(Notice::StoppedSilence),
            Some(StopReason::Limit) => Some(Notice::StoppedLimit),
            _ if !t.wrote => Some(Notice::NothingHeard),
            _ => None,
        };
        if let Some(n) = notice {
            self.notices.retain(|(e, _)| *e != t.editor);
            self.notices.push((t.editor, n));
        }
        out.push(Output::Done {
            editor: t.editor,
            take: t.id,
        });
    }

    fn send(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Waits until the engine thread has answered everything sent so far
    /// or `limit` passes, then polls. For tests and shutdown.
    pub fn poll_until_settled(
        &mut self,
        editors: &dyn Editors,
        limit: std::time::Duration,
    ) -> Vec<Output> {
        let deadline = std::time::Instant::now() + limit;
        let mut out = Vec::new();
        loop {
            out.extend(self.poll(editors));
            let busy = self.finishing.iter().any(|t| !t.pending.is_empty())
                || self
                    .active
                    .as_ref()
                    .is_some_and(|t| !t.pending.is_empty() || (!t.engine_started && !t.stopped));
            if !busy || std::time::Instant::now() >= deadline {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.dropped_below.store(u64::MAX, Ordering::SeqCst);
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_engine(
    mut engine: Box<dyn Engine>,
    jobs: Receiver<Job>,
    outcomes: Sender<Outcome>,
    dropped_below: Arc<AtomicU64>,
) {
    let mut running = false;
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Start { take, lang } => {
                if running {
                    engine.stop();
                }
                let result = engine.start(lang);
                running = result.is_ok();
                let _ = outcomes.send(Outcome::Started { take, result });
            }
            Job::Feed {
                take,
                seq,
                mut phrase,
            } => {
                if take < dropped_below.load(Ordering::SeqCst) {
                    phrase.wipe();
                    continue;
                }
                let result = engine.feed(phrase.as_slice());
                phrase.wipe();
                let _ = outcomes.send(Outcome::Text { take, seq, result });
            }
            Job::Release(done) => {
                engine.release();
                running = false;
                let _ = done.send(());
            }
            Job::Stop => {
                // Every phrase of the stopped take was queued before this
                // job, so it has been decoded (or skipped) by now.
                if running {
                    engine.stop();
                    running = false;
                }
            }
        }
    }
    if running {
        engine.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::engine::{FakeEngine, FakeStep};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_secs(5);

    /// Editors as plain strings, the caret at the end unless set.
    #[derive(Default)]
    struct Texts(RefCell<HashMap<EditorId, (String, usize)>>);

    impl Texts {
        fn with(editors: &[(EditorId, &str)]) -> Self {
            let t = Texts::default();
            for (id, s) in editors {
                t.0.borrow_mut().insert(*id, (s.to_string(), s.len()));
            }
            t
        }

        fn apply(&self, outputs: &[Output]) {
            for o in outputs {
                if let Output::Insert {
                    editor, insertion, ..
                } = o
                {
                    let mut map = self.0.borrow_mut();
                    let (text, caret) = map.get_mut(editor).unwrap();
                    text.insert_str(insertion.at, &insertion.text);
                    if let Some(c) = insertion.caret {
                        *caret = c;
                    }
                }
            }
        }

        fn get(&self, id: EditorId) -> String {
            self.0.borrow()[&id].0.clone()
        }

        /// The person types at the end of `id`; returns (at, len).
        fn type_at_end(&self, id: EditorId, s: &str) -> (usize, usize) {
            let mut map = self.0.borrow_mut();
            let (text, _) = map.get_mut(&id).unwrap();
            let at = text.len();
            text.push_str(s);
            (at, s.len())
        }
    }

    struct View<'a>(std::cell::Ref<'a, HashMap<EditorId, (String, usize)>>);

    impl Editors for View<'_> {
        fn text_and_caret(&self, editor: EditorId) -> Option<(&str, usize)> {
            self.0.get(&editor).map(|(t, c)| (t.as_str(), *c))
        }
    }

    fn settle(c: &mut Controller, texts: &Texts) -> Vec<Output> {
        let out = {
            let view = View(texts.0.borrow());
            c.poll_until_settled(&view, WAIT)
        };
        texts.apply(&out);
        out
    }

    fn phrase() -> AudioBuf {
        let mut b = AudioBuf::new();
        b.extend_from_slice(&[0.1; 1600]);
        b
    }

    fn fake(script: Vec<FakeStep>) -> Controller {
        Controller::new(Box::new(FakeEngine::new(script)))
    }

    fn text(s: &str) -> FakeStep {
        FakeStep::Text(s.into())
    }

    #[test]
    fn a_take_goes_through_its_states_and_writes_its_phrases() {
        let texts = Texts::with(&[(1, "Notes for Alexandre:")]);
        let mut c = fake(vec![text(" First."), text(" Second.")]);
        assert_eq!(c.state(1), TakeState::Idle);
        let take = c.start(1, texts.get(1).len(), Lang::English);
        assert_eq!(c.state(1), TakeState::Starting);
        settle(&mut c, &texts);
        assert_eq!(c.state(1), TakeState::Starting, "until capture runs");
        c.capture_started();
        assert_eq!(c.state(1), TakeState::Listening);
        c.phrase(phrase());
        c.phrase(phrase());
        let out = settle(&mut c, &texts);
        assert_eq!(out.len(), 2);
        assert!(
            out.iter()
                .all(|o| matches!(o, Output::Insert { take: t, .. } if *t == take))
        );
        assert_eq!(texts.get(1), "Notes for Alexandre: First. Second.");
        c.stop(StopReason::Asked);
        let out = settle(&mut c, &texts);
        assert_eq!(out, vec![Output::Done { editor: 1, take }]);
        assert_eq!(c.state(1), TakeState::Idle);
        assert_eq!(c.notice(1), None);
    }

    #[test]
    fn a_stalled_engine_keeps_the_take_writing_and_esc_drops_it() {
        let texts = Texts::with(&[(1, "")]);
        let mut c = fake(vec![
            text("Kept."),
            FakeStep::Stall(Duration::from_millis(300), "Dropped.".into()),
            text("Also dropped."),
        ]);
        c.start(1, 0, Lang::French);
        c.capture_started();
        c.phrase(phrase());
        settle(&mut c, &texts);
        c.phrase(phrase());
        c.phrase(phrase());
        c.stop(StopReason::Asked);
        {
            let view = View(texts.0.borrow());
            let _ = c.poll(&view);
        }
        assert_eq!(c.state(1), TakeState::Writing);
        c.escape(1);
        assert_eq!(c.state(1), TakeState::Idle);
        std::thread::sleep(Duration::from_millis(500));
        let out = settle(&mut c, &texts);
        assert!(out.is_empty(), "{out:?}");
        assert_eq!(texts.get(1), "Kept.");
    }

    #[test]
    fn a_crash_mid_take_keeps_written_text() {
        let texts = Texts::with(&[(1, "Draft.")]);
        let mut c = fake(vec![
            text("Before the crash."),
            FakeStep::Exit,
            text("Never."),
        ]);
        c.start(1, 6, Lang::Detect);
        c.capture_started();
        c.phrase(phrase());
        c.phrase(phrase());
        c.phrase(phrase());
        let out = settle(&mut c, &texts);
        assert!(out.contains(&Output::Done { editor: 1, take: 1 }));
        assert_eq!(texts.get(1), "Draft. Before the crash.");
        assert_eq!(c.state(1), TakeState::Error(EngineError::Exited));
        assert_eq!(c.active_editor(), None);
        // The next take in that editor clears the error, then meets the
        // same dead engine.
        c.start(1, 0, Lang::Detect);
        assert_eq!(c.state(1), TakeState::Starting);
        settle(&mut c, &texts);
        assert_eq!(c.state(1), TakeState::Error(EngineError::Exited));
    }

    #[test]
    fn a_failed_phrase_is_skipped_and_the_take_goes_on() {
        let texts = Texts::with(&[(1, "")]);
        let mut c = fake(vec![text("One."), FakeStep::Fail, text("Three.")]);
        c.start(1, 0, Lang::English);
        c.capture_started();
        for _ in 0..3 {
            c.phrase(phrase());
        }
        settle(&mut c, &texts);
        assert_eq!(texts.get(1), "One. Three.");
        assert_eq!(c.state(1), TakeState::Listening);
    }

    #[test]
    fn a_second_editor_stops_the_first_whose_last_phrase_still_lands_there() {
        let texts = Texts::with(&[(1, "First editor."), (2, "Second editor.")]);
        let mut c = fake(vec![
            FakeStep::Stall(Duration::from_millis(200), "Last words.".into()),
            text("New take."),
        ]);
        let first = c.start(1, texts.get(1).len(), Lang::English);
        c.capture_started();
        c.phrase(phrase());
        let second = c.start(2, texts.get(2).len(), Lang::English);
        assert_eq!(c.state(1), TakeState::Writing);
        assert_eq!(c.state(2), TakeState::Starting);
        c.capture_started();
        c.phrase(phrase());
        let out = settle(&mut c, &texts);
        assert!(out.contains(&Output::Done {
            editor: 1,
            take: first
        }));
        assert_eq!(texts.get(1), "First editor. Last words.");
        assert_eq!(texts.get(2), "Second editor. New take.");
        assert_eq!(c.state(1), TakeState::Idle);
        assert_eq!(c.state(2), TakeState::Listening);
        assert_eq!(c.active_editor(), Some(2));
        let _ = second;
    }

    #[test]
    fn focus_loss_stops_and_writes_the_last_phrase_to_its_own_editor() {
        let texts = Texts::with(&[(1, "")]);
        let mut c = fake(vec![FakeStep::Stall(
            Duration::from_millis(100),
            "Said before leaving.".into(),
        )]);
        c.start(1, 0, Lang::English);
        c.capture_started();
        c.phrase(phrase());
        c.stop(StopReason::FocusLost);
        assert_eq!(c.state(1), TakeState::Writing);
        settle(&mut c, &texts);
        assert_eq!(texts.get(1), "Said before leaving.");
        assert_eq!(c.state(1), TakeState::Idle);
    }

    #[test]
    fn edits_during_a_take_move_the_anchor() {
        let texts = Texts::with(&[(1, "Start. End.")]);
        let mut c = fake(vec![text("Middle.")]);
        c.start(1, 6, Lang::English);
        c.capture_started();
        // The person types at the end, after the anchor; the caret is there.
        let (at, len) = texts.type_at_end(1, " More.");
        c.edited(1, at, 0, len);
        {
            let mut map = texts.0.borrow_mut();
            let entry = map.get_mut(&1).unwrap();
            entry.1 = entry.0.len();
        }
        c.phrase(phrase());
        let out = settle(&mut c, &texts);
        assert_eq!(texts.get(1), "Start. Middle. End. More.");
        let Output::Insert { insertion, .. } = &out[0] else {
            panic!("{out:?}");
        };
        assert_eq!(insertion.caret, None, "the caret was elsewhere");
    }

    #[test]
    fn nothing_heard_silence_and_limit_notices() {
        let texts = Texts::with(&[(1, ""), (2, ""), (3, "")]);
        let mut c = fake(vec![text("   ")]);
        c.start(1, 0, Lang::English);
        c.capture_started();
        c.phrase(phrase());
        c.stop(StopReason::Asked);
        settle(&mut c, &texts);
        assert_eq!(c.notice(1), Some(Notice::NothingHeard));
        c.start(2, 0, Lang::English);
        c.stop(StopReason::Silence);
        settle(&mut c, &texts);
        assert_eq!(c.notice(2), Some(Notice::StoppedSilence));
        c.start(3, 0, Lang::English);
        c.stop(StopReason::Limit);
        settle(&mut c, &texts);
        assert_eq!(c.notice(3), Some(Notice::StoppedLimit));
        // A new take clears the line.
        c.start(1, 0, Lang::English);
        assert_eq!(c.notice(1), None);
    }

    #[test]
    fn a_start_failure_is_a_model_error() {
        let texts = Texts::with(&[(1, "")]);
        let mut engine = FakeEngine::new(vec![]);
        engine.fail_start = true;
        let mut c = Controller::new(Box::new(engine));
        c.start(1, 0, Lang::English);
        settle(&mut c, &texts);
        assert_eq!(c.state(1), TakeState::Error(EngineError::Model));
        assert_eq!(c.active_editor(), None);
        c.closed(1);
        assert_eq!(c.state(1), TakeState::Idle);
    }

    #[test]
    fn releasing_the_engine_stops_takes_and_waits_for_it() {
        let texts = Texts::with(&[(1, "")]);
        let engine = FakeEngine::new(vec![FakeStep::Stall(
            Duration::from_millis(200),
            "Dropped.".into(),
        )]);
        let released = std::sync::Arc::clone(&engine.released);
        let mut c = Controller::new(Box::new(engine));
        c.start(1, 0, Lang::English);
        c.capture_started();
        c.phrase(phrase());
        // Waits behind the stalled phrase, then confirms.
        assert!(c.release_engine(WAIT));
        assert_eq!(released.load(Ordering::SeqCst), 1);
        assert_eq!(c.active_editor(), None);
        assert_eq!(c.state(1), TakeState::Idle);
        assert!(settle(&mut c, &texts).is_empty());
        assert_eq!(texts.get(1), "");
    }

    #[test]
    fn phrases_of_a_dropped_take_are_wiped_not_decoded() {
        let texts = Texts::with(&[(1, "")]);
        let mut c = fake(vec![FakeStep::Stall(
            Duration::from_millis(200),
            "One.".into(),
        )]);
        c.start(1, 0, Lang::English);
        c.capture_started();
        c.phrase(phrase());
        c.phrase(phrase());
        c.phrase(phrase());
        c.escape(1);
        std::thread::sleep(Duration::from_millis(400));
        assert!(settle(&mut c, &texts).is_empty());
        assert_eq!(texts.get(1), "");
    }
}
