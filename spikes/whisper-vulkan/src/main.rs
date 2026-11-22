//! Spike for ADR 0014 step 2: Whisper on a Vulkan GPU when there is one, on the
//! CPU otherwise, from one executable.
//!
//! ggml-vulkan is linked statically. Its only direct call into the Vulkan
//! loader is `vkGetInstanceProcAddr` (everything else goes through Vulkan-Hpp's
//! dynamic dispatcher). This binary defines that symbol itself, so the linker
//! resolves it here and never imports `vulkan-1.dll` / `libvulkan.so.1`. The
//! shim loads the real loader at run time; when it is missing (or disabled), it
//! answers `vkEnumerateInstanceVersion` with Vulkan 1.0, which ggml rejects
//! ("Vulkan 1.2 required") with a C++ exception it catches itself, so the
//! Vulkan backend is simply not registered and Whisper runs on the CPU.
//!
//! Usage: whisper-vulkan-spike <model> <wav 16 kHz mono s16> <cpu|gpu> [runs]
//! `SPIKE_LOADER` overrides the loader's file name (to simulate a missing one).

use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[cfg(feature = "vulkan")]
mod shim {
    use std::ffi::{c_char, c_void, CStr};
    use std::sync::OnceLock;

    type Pfn = unsafe extern "system" fn();
    type GetProc = unsafe extern "system" fn(*mut c_void, *const c_char) -> Option<Pfn>;

    struct Loader {
        get_instance_proc_addr: GetProc,
        cmd_copy_buffer: unsafe extern "system" fn(*mut c_void, u64, u64, u32, *const c_void),
        get_physical_device_features2: unsafe extern "system" fn(*mut c_void, *mut c_void),
    }

    #[cfg(windows)]
    fn load_real() -> Option<Loader> {
        #[link(name = "kernel32")]
        extern "system" {
            fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        }
        const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;
        let name = std::env::var("SPIKE_LOADER").unwrap_or_else(|_| "vulkan-1.dll".into());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: valid NUL-terminated strings; System32 only, never the app dir.
        unsafe {
            let module = LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            );
            if module.is_null() {
                return None;
            }
            let get = |n: &CStr| {
                let f = GetProcAddress(module, n.as_ptr());
                (!f.is_null()).then_some(f)
            };
            Some(Loader {
                get_instance_proc_addr: std::mem::transmute(get(c"vkGetInstanceProcAddr")?),
                cmd_copy_buffer: std::mem::transmute(get(c"vkCmdCopyBuffer")?),
                get_physical_device_features2: std::mem::transmute(get(
                    c"vkGetPhysicalDeviceFeatures2",
                )?),
            })
        }
    }

    fn real() -> Option<&'static Loader> {
        static REAL: OnceLock<Option<Loader>> = OnceLock::new();
        REAL.get_or_init(|| {
            let r = load_real();
            eprintln!(
                "spike: Vulkan loader {}",
                if r.is_some() {
                    "found"
                } else {
                    "missing, CPU only"
                }
            );
            r
        })
        .as_ref()
    }

    // The two commands ggml-vulkan calls directly rather than through its
    // dispatcher. Reached only after a successful instance init, so the
    // loader is there.
    #[no_mangle]
    pub unsafe extern "system" fn vkCmdCopyBuffer(
        cb: *mut c_void,
        src: u64,
        dst: u64,
        n: u32,
        regions: *const c_void,
    ) {
        // SAFETY: same arguments as the loader's export.
        unsafe { (real().expect("loader").cmd_copy_buffer)(cb, src, dst, n, regions) }
    }

    #[no_mangle]
    pub unsafe extern "system" fn vkGetPhysicalDeviceFeatures2(
        dev: *mut c_void,
        features: *mut c_void,
    ) {
        // SAFETY: same arguments as the loader's export.
        unsafe { (real().expect("loader").get_physical_device_features2)(dev, features) }
    }

    unsafe extern "system" fn version_1_0(out: *mut u32) -> i32 {
        // SAFETY: Vulkan-Hpp passes a valid pointer.
        unsafe { *out = 1 << 22 };
        0
    }

    /// # Safety
    /// Called by ggml-vulkan with the Vulkan loader's contract.
    #[no_mangle]
    pub unsafe extern "system" fn vkGetInstanceProcAddr(
        instance: *mut c_void,
        name: *const c_char,
    ) -> Option<Pfn> {
        match real() {
            // SAFETY: forwarding the same arguments to the real loader.
            Some(l) => unsafe { (l.get_instance_proc_addr)(instance, name) },
            None => {
                // SAFETY: `name` is a NUL-terminated string from the caller.
                let n = unsafe { CStr::from_ptr(name) };
                (n == c"vkEnumerateInstanceVersion").then(|| {
                    // SAFETY: called back with its real signature by Vulkan-Hpp.
                    unsafe {
                        std::mem::transmute::<unsafe extern "system" fn(*mut u32) -> i32, Pfn>(
                            version_1_0,
                        )
                    }
                })
            }
        }
    }
}

fn read_wav(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).expect("wav");
    // Find the "data" chunk; accept only PCM s16 mono 16 kHz.
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            let ch = u16::from_le_bytes([b[i + 10], b[i + 11]]);
            let rate = u32::from_le_bytes(b[i + 12..i + 16].try_into().unwrap());
            let bits = u16::from_le_bytes([b[i + 22], b[i + 23]]);
            assert!(
                ch == 1 && rate == 16000 && bits == 16,
                "need s16 mono 16 kHz, got {ch} ch {rate} Hz {bits} bit"
            );
        }
        if id == b"data" {
            let end = (i + 8 + len).min(b.len());
            return b[i + 8..end]
                .chunks_exact(2)
                .map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0)
                .collect();
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (model, wav, gpu) = (&a[1], &a[2], a[3] == "gpu");
    let runs: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(3);
    let audio = read_wav(wav);
    eprintln!(
        "spike: {:.2} s of audio, use_gpu={gpu}",
        audio.len() as f32 / 16000.0
    );

    let t = Instant::now();
    let mut cp = WhisperContextParameters::default();
    cp.use_gpu(gpu);
    let ctx = WhisperContext::new_with_params(model, cp).expect("model");
    let mut state = ctx.create_state().expect("state");
    eprintln!("spike: load {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);

    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
        .clamp(1, 8);
    let mut times = Vec::new();
    for _ in 0..runs {
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_n_threads(threads);
        p.set_language(Some("en"));
        p.set_no_timestamps(true);
        p.set_temperature_inc(0.0);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_special(false);
        p.set_print_timestamps(false);
        let t = Instant::now();
        state.full(p, &audio).expect("full");
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let text: String = (0..state.full_n_segments())
        .filter_map(|i| {
            state
                .get_segment(i)
                .and_then(|s| s.to_str_lossy().ok().map(|c| c.into_owned()))
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "mode={} threads={threads} runs_ms={times:.0?} median_ms={:.0}",
        if gpu { "gpu" } else { "cpu" },
        times[times.len() / 2]
    );
    println!("text={}", text.trim());
}
