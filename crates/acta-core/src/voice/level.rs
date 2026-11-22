//! The input level for the bar (components-voice.md 5): the RMS of the
//! last 50 ms on a decibel scale from -60 to 0 dBFS.

use super::samples;

/// The bar's floor: anything quieter shows an empty bar.
pub const FLOOR_DBFS: f32 = -60.0;

/// The RMS of `samples` in dBFS, clamped to [-60, 0]. Silence (or nothing)
/// is the floor.
pub fn dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return FLOOR_DBFS;
    }
    let mean_square =
        samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64;
    if mean_square <= 0.0 {
        return FLOOR_DBFS;
    }
    let db = 10.0 * mean_square.log10();
    (db as f32).clamp(FLOOR_DBFS, 0.0)
}

/// The bar's fill, 0 to 1, for a level in dBFS.
pub fn bar_fraction(dbfs: f32) -> f32 {
    ((dbfs - FLOOR_DBFS) / -FLOOR_DBFS).clamp(0.0, 1.0)
}

/// Accumulates 16 kHz samples and yields a level every 50 ms. Keeps only
/// running sums, never the samples.
#[derive(Debug, Default)]
pub struct LevelMeter {
    sum: f64,
    count: usize,
}

/// Samples in one level window.
pub const WINDOW: usize = samples(50);

impl LevelMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds samples; returns the level of the last full 50 ms window
    /// completed by them, if any.
    pub fn push(&mut self, samples: &[f32]) -> Option<f32> {
        let mut last = None;
        for s in samples {
            self.sum += f64::from(*s).powi(2);
            self.count += 1;
            if self.count == WINDOW {
                let ms = self.sum / WINDOW as f64;
                last = Some(if ms <= 0.0 {
                    FLOOR_DBFS
                } else {
                    ((10.0 * ms.log10()) as f32).clamp(FLOOR_DBFS, 0.0)
                });
                self.sum = 0.0;
                self.count = 0;
            }
        }
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| amplitude * (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin())
            .collect()
    }

    #[test]
    fn full_scale_sine_is_minus_three_dbfs() {
        let level = dbfs(&sine(1.0, WINDOW));
        assert!((level - -3.01).abs() < 0.05, "{level}");
    }

    #[test]
    fn levels_clamp_to_the_bar() {
        assert_eq!(dbfs(&[]), FLOOR_DBFS);
        assert_eq!(dbfs(&[0.0; 800]), FLOOR_DBFS);
        assert_eq!(dbfs(&sine(0.0001, 800)), FLOOR_DBFS);
        assert_eq!(dbfs(&[1.0; 800]), 0.0);
        assert_eq!(bar_fraction(-60.0), 0.0);
        assert_eq!(bar_fraction(-30.0), 0.5);
        assert_eq!(bar_fraction(0.0), 1.0);
        assert_eq!(bar_fraction(-90.0), 0.0);
    }

    #[test]
    fn the_meter_reports_every_fifty_ms() {
        let mut meter = LevelMeter::new();
        assert_eq!(meter.push(&sine(0.5, WINDOW - 1)), None);
        let level = meter.push(&sine(0.5, 1)).unwrap();
        assert!((level - -9.03).abs() < 0.1, "{level}");
        // 120 ms: two windows; the last one is reported.
        let level = meter.push(&[0.0; 2 * WINDOW + 10]).unwrap();
        assert_eq!(level, FLOOR_DBFS);
    }
}
