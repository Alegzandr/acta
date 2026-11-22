//! The machine as the model recommendation sees it (ADR 0014, 4,
//! "Detection"): total memory and physical cores from `sysinfo`, the
//! processor's features at run time, Metal on an Apple Silicon build.
//! Blocking for a few milliseconds: call it off the UI thread.

use acta_core::voice::hardware::{Cpu, Machine, this_cpu};

pub fn detect() -> Machine {
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
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_machine_has_memory_and_cores() {
        let m = super::detect();
        assert!(m.memory > 0);
        assert!(m.physical_cores >= 1);
    }
}
