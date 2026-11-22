//! The audio buffer: a `Vec<f32>` whose whole allocation is zeroed when it
//! is cleared, reallocated or dropped (ADR 0014, 7). Every buffer of
//! samples in the dictation path is one of these, so no phrase is left in
//! freed memory.

use zeroize::Zeroize;

/// Samples that are wiped when they are no longer needed.
#[derive(Default)]
pub struct AudioBuf(Vec<f32>);

impl AudioBuf {
    pub fn new() -> Self {
        AudioBuf(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        AudioBuf(Vec::with_capacity(capacity))
    }

    /// Takes ownership of samples decoded elsewhere (the worker's input).
    pub fn from_vec(samples: Vec<f32>) -> Self {
        AudioBuf(samples)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// Appends without ever leaving a copy behind: when the vector must
    /// grow, the old allocation is zeroed before it is freed.
    pub fn extend_from_slice(&mut self, samples: &[f32]) {
        self.reserve(samples.len());
        self.0.extend_from_slice(samples);
    }

    pub fn push(&mut self, sample: f32) {
        self.reserve(1);
        self.0.push(sample);
    }

    fn reserve(&mut self, more: usize) {
        let needed = self.0.len() + more;
        if needed <= self.0.capacity() {
            return;
        }
        let mut grown = Vec::with_capacity(needed.max(self.0.capacity() * 2).max(256));
        grown.extend_from_slice(&self.0);
        let mut old = std::mem::replace(&mut self.0, grown);
        wipe_vec(&mut old);
    }

    /// Removes the first `n` samples; the vacated tail is zeroed.
    pub fn drain_front(&mut self, n: usize) {
        let n = n.min(self.0.len());
        let len = self.0.len();
        self.0.copy_within(n.., 0);
        self.0[len - n..].zeroize();
        self.0.truncate(len - n);
    }

    /// Moves the samples out into a new buffer and leaves this one empty
    /// (its allocation zeroed and kept).
    pub fn take(&mut self) -> AudioBuf {
        let mut out = AudioBuf::with_capacity(self.0.len());
        out.0.extend_from_slice(&self.0);
        self.wipe();
        out
    }

    /// Zeroes every sample and the spare capacity, and empties the buffer.
    pub fn wipe(&mut self) {
        wipe_vec(&mut self.0);
    }

    /// Test hook: whether the whole allocation, spare capacity included, is
    /// zero. Reads only memory this buffer owns and has initialised.
    /// Only meaningful after `wipe`, which writes zeros to all of it.
    #[cfg(test)]
    pub(crate) fn allocation_is_zero(&self) -> bool {
        let capacity = self.0.capacity();
        // SAFETY: [0, capacity) is this vector's allocation; `wipe` (or the
        // pushes before it) initialised every element of it.
        let all = unsafe { std::slice::from_raw_parts(self.0.as_ptr() as *const u32, capacity) };
        all.iter().all(|bits| *bits == 0)
    }
}

fn wipe_vec(v: &mut Vec<f32>) {
    #[cfg(test)]
    tests::count_wipe(v.len());
    // `Zeroize for Vec` zeroes the elements, then the spare capacity, then
    // clears.
    v.zeroize();
}

/// Shows the length only, never the samples.
impl std::fmt::Debug for AudioBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AudioBuf({} samples)", self.0.len())
    }
}

impl Drop for AudioBuf {
    fn drop(&mut self) {
        self.wipe();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static WIPED: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn count_wipe(len: usize) {
        WIPED.with(|w| w.set(w.get() + len));
    }

    /// Samples wiped on this thread since the last call.
    pub(crate) fn wiped_since_last() -> usize {
        WIPED.with(|w| w.replace(0))
    }

    #[test]
    fn wipe_zeroes_the_whole_allocation() {
        let mut b = AudioBuf::with_capacity(1000);
        b.extend_from_slice(&[0.5; 700]);
        b.wipe();
        assert!(b.is_empty());
        assert!(b.0.capacity() >= 1000);
        assert!(b.allocation_is_zero());
    }

    #[test]
    fn growing_wipes_the_old_allocation_and_drop_wipes_the_rest() {
        let _ = wiped_since_last();
        let mut b = AudioBuf::new();
        for _ in 0..10 {
            b.extend_from_slice(&[0.25; 300]);
        }
        let grown = wiped_since_last();
        assert!(grown > 0, "reallocations wipe what they leave");
        drop(b);
        assert_eq!(wiped_since_last(), 3000);
    }

    #[test]
    fn drain_front_and_take_leave_no_samples_behind() {
        let mut b = AudioBuf::new();
        b.extend_from_slice(&[1.0, 2.0, 3.0, 4.0]);
        b.drain_front(3);
        assert_eq!(b.as_slice(), &[4.0]);
        let taken = b.take();
        assert_eq!(taken.as_slice(), &[4.0]);
        assert!(b.is_empty());
        assert!(b.allocation_is_zero());
    }
}
