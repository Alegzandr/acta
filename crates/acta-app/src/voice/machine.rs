//! The machine as the model recommendation sees it (ADR 0014, 4,
//! "Detection"): total memory and physical cores from `sysinfo`, the
//! processor's features at run time, Metal on an Apple Silicon build, and on
//! Windows and Linux the Vulkan graphics card as a worker reports it (never
//! looked at in this process: no GPU driver is loaded into the GUI).
//! Blocking (up to [`PROBE_TIMEOUT`] when a driver hangs; usually well under
//! a second): call it off the UI thread.

use std::path::Path;
use std::time::Duration;

use acta_core::voice::hardware::{Cpu, Machine, this_cpu, whisper_runs};

/// How long the graphics card probe may take before it counts as none.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub fn detect() -> Machine {
    detect_with(std::env::current_exe().ok().as_deref())
}

/// `worker`: the executable to probe the graphics card with (this one);
/// `None` skips the probe.
fn detect_with(worker: Option<&Path>) -> Machine {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    let memory = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    )
    .map(|s| s.total_memory())
    .unwrap_or(0);
    let physical_cores = System::physical_core_count()
        .ok()
        .or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()))
        .unwrap_or(1) as u32;
    let cpu = this_cpu();
    Machine {
        memory,
        physical_cores,
        cpu,
        // The `metal` feature is on for this build only (Cargo.toml); an
        // x86 build under Rosetta sees an x86 processor.
        metal: cfg!(all(target_os = "macos", target_arch = "aarch64")) && cpu == Cpu::Arm64,
        vulkan_memory: worker.map_or(0, |exe| vulkan_memory(exe, cpu)),
    }
}

/// The graphics card's dedicated memory, from a worker started to probe it;
/// 0 where the worker cannot run (no AVX2: it would not even get that far),
/// on macOS, or when the probe fails or hangs.
fn vulkan_memory(exe: &Path, cpu: Cpu) -> u64 {
    if !cfg!(any(windows, target_os = "linux")) || !whisper_runs(cpu) {
        return 0;
    }
    acta_core::voice::worker::probe_gpu(exe, PROBE_TIMEOUT).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_machine_has_memory_and_cores() {
        let m = super::detect_with(None);
        assert!(m.memory > 0);
        assert!(m.physical_cores >= 1);
        assert_eq!(m.vulkan_memory, 0);
    }
}
