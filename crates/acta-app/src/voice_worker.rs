//! The dictation worker (ADR 0014, 5, and its "embedded only" addendum):
//! this executable started with `--voice-worker`, before any window, lock
//! or update step. It loads a Whisper model, turns the phrases it reads on
//! stdin into text on stdout, and exits when Acta closes the pipe, when it
//! has been idle for five minutes, or when its engine is gone.
//!
//! Hardening: no heap in Windows error reports, not dumpable on Linux,
//! whisper.cpp's logs dropped, stderr discarded by the parent, and the C
//! runtime's stdout pointed at the null device so nothing printed by native
//! code can corrupt the protocol (it runs on a private copy of the handle).

use std::fs::File;
use std::path::Path;

use acta_core::voice::engine::{Engine, EngineError, FakeEngine, Lang};
use acta_core::voice::protocol::{EXIT_ENGINE_GONE, EngineSpec};
use acta_core::voice::worker::{ServeEnd, serve};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};
use zeroize::Zeroize;

/// Runs the worker; the process exit code.
pub fn run() -> i32 {
    harden();
    // Without the `log` or `tracing` features these hooks drop every line.
    whisper_rs::install_logging_hooks();
    let Some(output) = private_stdout() else {
        return 2;
    };
    match serve(std::io::stdin(), output, make) {
        ServeEnd::Closed | ServeEnd::Idle => 0,
        ServeEnd::EngineGone => EXIT_ENGINE_GONE,
        ServeEnd::Broken => 2,
    }
}

fn make(spec: &EngineSpec) -> Result<Box<dyn Engine>, EngineError> {
    match spec {
        EngineSpec::Whisper {
            model,
            vad,
            threads,
            gpu,
        } => Ok(Box::new(WhisperEngine::load(
            model,
            vad.as_deref(),
            *threads,
            *gpu,
        )?)),
        EngineSpec::Fake { script } => Ok(Box::new(FakeEngine::new(script.clone()))),
    }
}

/// Crash reports without memory contents.
fn harden() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ErrorReporting::{
            WER_FAULT_REPORTING_FLAG_NOHEAP, WerSetFlags,
        };
        // SAFETY: a flag on this process; no pointers.
        unsafe {
            WerSetFlags(WER_FAULT_REPORTING_FLAG_NOHEAP);
        }
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: a flag on this process; no pointers.
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 0);
            // The parent set the death signal before exec; if it is
            // already gone, there is nobody to serve.
            if libc::getppid() == 1 {
                libc::_exit(0);
            }
        }
    }
}

/// A private copy of stdout for the protocol, with stdout itself (the OS
/// handle and the C runtime's descriptor 1) pointed at the null device.
fn private_stdout() -> Option<File> {
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle as _;
        use windows_sys::Win32::Foundation::{
            DUPLICATE_SAME_ACCESS, DuplicateHandle, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
        };
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_OUTPUT_HANDLE, SetStdHandle};
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        // SAFETY: handles are checked before use; the duplicate is owned by
        // the returned File; the NUL handle is handed to the process's
        // standard output for good.
        unsafe {
            let out = GetStdHandle(STD_OUTPUT_HANDLE);
            if out.is_null() || out == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut copy: HANDLE = std::ptr::null_mut();
            let me = GetCurrentProcess();
            if DuplicateHandle(me, out, me, &mut copy, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                return None;
            }
            let nul: Vec<u16> = "NUL\0".encode_utf16().collect();
            let null = CreateFileW(
                nul.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if null != INVALID_HANDLE_VALUE {
                SetStdHandle(STD_OUTPUT_HANDLE, null);
            }
            let fd = libc::open(c"NUL".as_ptr(), libc::O_WRONLY);
            if fd >= 0 {
                libc::dup2(fd, 1);
                libc::close(fd);
            }
            Some(File::from_raw_handle(copy))
        }
    }
    #[cfg(unix)]
    {
        use std::os::fd::FromRawFd as _;
        // SAFETY: plain descriptor calls; the duplicate is owned by the
        // returned File.
        unsafe {
            let copy = libc::dup(1);
            if copy < 0 {
                return None;
            }
            libc::fcntl(copy, libc::F_SETFD, libc::FD_CLOEXEC);
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            if null >= 0 {
                libc::dup2(null, 1);
                libc::close(null);
            }
            Some(File::from_raw_fd(copy))
        }
    }
}

/// whisper.cpp through whisper-rs (ADR 0014, 5, "Per phrase").
struct WhisperEngine {
    ctx: WhisperContext,
    /// One state per take: decoding with `no_context` off carries the
    /// take's previous text as the prompt; a new take starts clean.
    state: Option<WhisperState>,
    vad: Option<(WhisperVadContext, String)>,
    threads: i32,
    /// The take's language once decided.
    lang: Option<&'static str>,
}

impl WhisperEngine {
    fn load(
        model: &Path,
        vad: Option<&Path>,
        threads: u32,
        gpu: bool,
    ) -> Result<Self, EngineError> {
        let model = model.to_str().ok_or(EngineError::Model)?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu(gpu);
        let ctx = WhisperContext::new_with_params(model, params).map_err(|_| EngineError::Model)?;
        let threads = threads.clamp(1, 8) as i32;
        let vad = match vad.and_then(Path::to_str) {
            Some(path) => {
                let mut p = WhisperVadContextParams::new();
                p.set_n_threads(1);
                p.set_use_gpu(false);
                let vad = WhisperVadContext::new(path, p).map_err(|_| EngineError::Model)?;
                Some((vad, path.to_string()))
            }
            None => None,
        };
        Ok(WhisperEngine {
            ctx,
            state: None,
            vad,
            threads,
            lang: None,
        })
    }

    /// Whether the VAD hears speech in the phrase (always true without one).
    fn has_speech(&mut self, phrase: &[f32]) -> Result<bool, EngineError> {
        let Some((vad, _)) = self.vad.as_mut() else {
            return Ok(true);
        };
        vad.detect_speech(phrase).map_err(|_| EngineError::Failed)?;
        let segments = vad
            .segments_from_probabilities(WhisperVadParams::new())
            .map_err(|_| EngineError::Failed)?;
        Ok(segments.num_segments() > 0)
    }

    /// The more likely of English and French for this phrase.
    fn detect(
        &self,
        state: &mut WhisperState,
        phrase: &[f32],
    ) -> Result<&'static str, EngineError> {
        state
            .pcm_to_mel(phrase, self.threads as usize)
            .map_err(|_| EngineError::Failed)?;
        let (_, probs) = state
            .lang_detect(0, self.threads as usize)
            .map_err(|_| EngineError::Failed)?;
        let p = |code| {
            whisper_rs::get_lang_id(code)
                .and_then(|id| probs.get(id as usize).copied())
                .unwrap_or(0.0)
        };
        Ok(if p("fr") > p("en") { "fr" } else { "en" })
    }
}

impl Engine for WhisperEngine {
    fn start(&mut self, lang: Lang) -> Result<(), EngineError> {
        self.state = Some(self.ctx.create_state().map_err(|_| EngineError::Model)?);
        self.lang = match lang {
            Lang::Detect => None,
            Lang::English => Some("en"),
            Lang::French => Some("fr"),
        };
        Ok(())
    }

    fn feed(&mut self, phrase: &[f32]) -> Result<String, EngineError> {
        // A phrase the VAD finds no speech in is never decoded: the main
        // guard against invented text.
        if !self.has_speech(phrase)? {
            return Ok(String::new());
        }
        let mut state = self.state.take().ok_or(EngineError::Failed)?;
        let result = self.decode(&mut state, phrase);
        self.state = Some(state);
        result
    }

    fn stop(&mut self) {
        self.state = None;
        self.lang = None;
    }
}

impl WhisperEngine {
    fn decode(&mut self, state: &mut WhisperState, phrase: &[f32]) -> Result<String, EngineError> {
        let lang = match self.lang {
            Some(l) => l,
            None => {
                let l = self.detect(state, phrase)?;
                self.lang = Some(l);
                l
            }
        };
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_n_threads(self.threads);
        p.set_language(Some(lang));
        p.set_translate(false);
        p.set_no_context(false);
        p.set_no_timestamps(true);
        p.set_temperature_inc(0.0);
        p.set_suppress_blank(true);
        p.set_print_special(false);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        if let Some((_, path)) = &self.vad {
            p.set_vad_model_path(Some(path));
            p.enable_vad(true);
        }
        state.full(p, phrase).map_err(|_| EngineError::Failed)?;
        let mut text = String::new();
        for segment in state.as_iter() {
            if let Ok(mut s) = segment.to_str_lossy().map(|c| c.into_owned()) {
                text.push_str(&s);
                s.zeroize();
            }
        }
        let trimmed = text.trim().to_string();
        text.zeroize();
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_build_checks_the_vad_against_the_core_constants() {
        let build = include_str!("../build.rs");
        let vad = acta_core::voice::models::SILERO_VAD;
        assert!(build.contains(&format!("const VAD_FILE: &str = \"{}\";", vad.file)));
        assert!(build.contains(&format!("const VAD_SHA256: &str = \"{}\";", vad.sha256)));
        let size = vad.size.to_string();
        let grouped = format!("{},{}", &size[..3], &size[3..]).replace(',', "_");
        assert!(build.contains(&format!("const VAD_SIZE: u64 = {grouped};")));
    }
}
