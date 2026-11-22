//! Which model to recommend (ADR 0014, 4). Pure over a [`Machine`] the app
//! fills from the system; the recommendation is shown, never applied by
//! itself.
//!
//! Memory thresholds are decimal gigabytes (10^9 bytes): an "8 GB" computer
//! reports a little less than 8 GiB once the firmware and graphics have
//! taken their share (about 7.8 GiB, 8.4e9 bytes), which is still over
//! 8e9, so it lands in the tier its label says.

use super::models::ModelId;

const GB: u64 = 1_000_000_000;

/// The processor as Whisper sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    /// x86-64 (including an x86 build under Rosetta on a Mac), with the
    /// features ggml's fast paths need.
    X86_64 {
        avx2: bool,
        fma: bool,
    },
    Arm64,
    Other,
}

/// What the recommendation looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// Total physical memory, in bytes.
    pub memory: u64,
    pub physical_cores: u32,
    pub cpu: Cpu,
    /// Whisper can use Metal: an arm64 build with the `metal` feature on
    /// Apple Silicon. An x86 build under Rosetta is `false` here and
    /// `Cpu::X86_64` above.
    pub metal: bool,
}

/// Why that model, one line in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Apple Silicon with 8 GB or more: the large model on the GPU.
    Gpu,
    /// 8 GB, 4 cores and a fast vector unit: Small on the processor.
    Cpu,
    /// 4 GB or more, below the CPU tier.
    WeakerCpu,
    /// 2 GB or more: the floor.
    Floor,
    /// Under 2 GB of memory: dictation cannot run.
    TooLittleMemory,
    /// An x86-64 processor without AVX2 and FMA (before 2013), or another
    /// architecture: the worker is built for x86-64 with them
    /// (`.cargo/config.toml`), so it cannot run here.
    UnsupportedCpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recommendation {
    /// `None` when nothing can run ([`Reason::TooLittleMemory`]).
    pub model: Option<ModelId>,
    pub reason: Reason,
}

/// The recommended model and its reason.
pub fn recommend(m: Machine) -> Recommendation {
    let (model, reason) = if !whisper_runs(m.cpu) {
        (None, Reason::UnsupportedCpu)
    } else if m.metal && m.memory >= 8 * GB {
        (Some(ModelId::LargeTurbo), Reason::Gpu)
    } else if m.memory >= 8 * GB && m.physical_cores >= 4 {
        (Some(ModelId::Small), Reason::Cpu)
    } else if m.memory >= 4 * GB {
        (Some(ModelId::Base), Reason::WeakerCpu)
    } else if m.memory >= 2 * GB {
        (Some(ModelId::Tiny), Reason::Floor)
    } else {
        (None, Reason::TooLittleMemory)
    };
    Recommendation { model, reason }
}

/// Whether this build's whisper.cpp runs on the processor: AVX2 and FMA
/// on x86-64 (compiled in, no run-time dispatch without ggml's dynamic
/// backends), any arm64.
pub fn whisper_runs(cpu: Cpu) -> bool {
    match cpu {
        Cpu::X86_64 { avx2, fma } => avx2 && fma,
        Cpu::Arm64 => true,
        Cpu::Other => false,
    }
}

/// The threads Whisper decodes with: physical cores minus one (the UI and
/// capture keep one), at least one, at most 8.
pub fn decode_threads(physical_cores: u32) -> u32 {
    physical_cores.saturating_sub(1).clamp(1, 8)
}

/// This process's processor, detected at run time. Under Rosetta the x86
/// build sees an x86 processor, as the ADR wants.
pub fn this_cpu() -> Cpu {
    #[cfg(target_arch = "x86_64")]
    {
        Cpu::X86_64 {
            avx2: std::arch::is_x86_feature_detected!("avx2"),
            fma: std::arch::is_x86_feature_detected!("fma"),
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        Cpu::Arm64
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        Cpu::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAST: Cpu = Cpu::X86_64 {
        avx2: true,
        fma: true,
    };

    fn m(memory: u64, cores: u32, cpu: Cpu, metal: bool) -> Machine {
        Machine {
            memory,
            physical_cores: cores,
            cpu,
            metal,
        }
    }

    fn pick(machine: Machine) -> (Option<ModelId>, Reason) {
        let r = recommend(machine);
        (r.model, r.reason)
    }

    const fn gb_tenths(tenths: u64) -> u64 {
        tenths * GB / 10
    }

    #[test]
    fn every_row_of_the_ladder() {
        use ModelId::*;
        // Large turbo: Apple Silicon, Metal, 8 GB or more.
        assert_eq!(
            pick(m(16 * GB, 8, Cpu::Arm64, true)),
            (Some(LargeTurbo), Reason::Gpu)
        );
        // Small: 8 GB, 4 cores, AVX2 and FMA (or arm64).
        assert_eq!(pick(m(8 * GB, 4, FAST, false)), (Some(Small), Reason::Cpu));
        assert_eq!(
            pick(m(8 * GB, 4, Cpu::Arm64, false)),
            (Some(Small), Reason::Cpu)
        );
        // Base: 4 GB or more, below the CPU tier.
        assert_eq!(
            pick(m(4 * GB, 2, FAST, false)),
            (Some(Base), Reason::WeakerCpu)
        );
        // Tiny: 2 GB or more.
        assert_eq!(pick(m(2 * GB, 2, FAST, false)), (Some(Tiny), Reason::Floor));
        // Nothing under 2 GB.
        assert_eq!(
            pick(m(gb_tenths(19), 4, FAST, false)),
            (None, Reason::TooLittleMemory)
        );
    }

    #[test]
    fn the_edges() {
        use ModelId::*;
        // 7.9 vs 8 GB.
        assert_eq!(pick(m(gb_tenths(79), 8, FAST, false)).0, Some(Base));
        assert_eq!(pick(m(8 * GB, 8, FAST, false)).0, Some(Small));
        // A Mac with 7.9 GB is not the GPU tier.
        assert_eq!(pick(m(gb_tenths(79), 8, Cpu::Arm64, true)).0, Some(Base));
        // 3 vs 4 cores.
        assert_eq!(pick(m(16 * GB, 3, FAST, false)).0, Some(Base));
        assert_eq!(pick(m(16 * GB, 4, FAST, false)).0, Some(Small));
        // No AVX2, or no FMA: this build's worker cannot run there.
        let no_avx2 = Cpu::X86_64 {
            avx2: false,
            fma: true,
        };
        let no_fma = Cpu::X86_64 {
            avx2: true,
            fma: false,
        };
        for cpu in [no_avx2, no_fma, Cpu::Other] {
            assert_eq!(
                pick(m(16 * GB, 8, cpu, false)),
                (None, Reason::UnsupportedCpu)
            );
            assert!(!whisper_runs(cpu));
        }
        // Rosetta: an x86 build on Apple Silicon has no Metal; with
        // Rosetta's AVX2 it is the CPU tier, without it nothing.
        assert_eq!(pick(m(16 * GB, 8, FAST, false)).0, Some(Small));
        assert_eq!(pick(m(16 * GB, 8, no_avx2, true)).0, None);
        // 3.9 GB: under Base, still Tiny; 1.9 GB: nothing.
        assert_eq!(pick(m(gb_tenths(39), 8, FAST, false)).0, Some(Tiny));
        assert_eq!(pick(m(gb_tenths(19), 8, Cpu::Arm64, true)).0, None);
        // An 8 GiB machine reporting 7.8 GiB is still an 8 GB machine.
        assert_eq!(pick(m(78 * (1 << 30) / 10, 4, FAST, false)).0, Some(Small));
    }

    #[test]
    fn decode_threads_are_cores_minus_one_within_one_and_eight() {
        assert_eq!(decode_threads(0), 1);
        assert_eq!(decode_threads(1), 1);
        assert_eq!(decode_threads(2), 1);
        assert_eq!(decode_threads(4), 3);
        assert_eq!(decode_threads(9), 8);
        assert_eq!(decode_threads(32), 8);
    }
}
