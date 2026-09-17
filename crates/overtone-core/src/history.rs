//! Ring buffer of recorded byte spectra, one per analysis tick, bounded by a
//! memory budget. Tick numbers keep counting up forever; the oldest ticks are
//! overwritten once the budget is reached.

pub struct History {
    bin_count: usize,
    capacity: usize,
    data: Vec<u8>,
    total: u64,
}

impl History {
    /// `budget_bytes` bounds the memory used; at least one tick is kept.
    pub fn new(bin_count: usize, budget_bytes: usize) -> Self {
        let bin_count = bin_count.max(1);
        Self {
            bin_count,
            capacity: (budget_bytes / bin_count).max(1),
            data: Vec::new(),
            total: 0,
        }
    }

    pub fn bin_count(&self) -> usize {
        self.bin_count
    }

    /// Maximum number of ticks retained.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of ticks ever recorded; the next tick gets this index.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Oldest tick still available.
    pub fn first(&self) -> u64 {
        self.total.saturating_sub(self.capacity as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Record one spectrum (`bin_count` bytes; shorter input is zero-padded,
    /// longer input is truncated).
    pub fn push(&mut self, spectrum: &[u8]) {
        let slot = (self.total % self.capacity as u64) as usize * self.bin_count;
        if self.data.len() < slot + self.bin_count {
            self.data.resize(slot + self.bin_count, 0);
        }
        let dst = &mut self.data[slot..slot + self.bin_count];
        let n = spectrum.len().min(self.bin_count);
        dst[..n].copy_from_slice(&spectrum[..n]);
        dst[n..].iter_mut().for_each(|b| *b = 0);
        self.total += 1;
    }

    /// Overwrite an existing tick in place. Returns false if it is not held.
    pub fn set(&mut self, tick: u64, spectrum: &[u8]) -> bool {
        if tick >= self.total || tick < self.first() {
            return false;
        }
        let slot = (tick % self.capacity as u64) as usize * self.bin_count;
        let dst = &mut self.data[slot..slot + self.bin_count];
        let n = spectrum.len().min(self.bin_count);
        dst[..n].copy_from_slice(&spectrum[..n]);
        dst[n..].iter_mut().for_each(|b| *b = 0);
        true
    }

    /// Append `n` silent ticks.
    pub fn push_silence(&mut self, n: u64) {
        let zeros = vec![0u8; self.bin_count];
        for _ in 0..n.min(self.capacity as u64) {
            self.push(&zeros);
        }
        self.total += n.saturating_sub(self.capacity as u64);
    }

    pub fn get(&self, tick: u64) -> Option<&[u8]> {
        if tick >= self.total || tick < self.first() {
            return None;
        }
        let slot = (tick % self.capacity as u64) as usize * self.bin_count;
        Some(&self.data[slot..slot + self.bin_count])
    }

    pub fn latest(&self) -> Option<&[u8]> {
        self.total.checked_sub(1).and_then(|t| self.get(t))
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_most_recent_ticks_within_budget() {
        let mut h = History::new(4, 4 * 10);
        assert_eq!(h.capacity(), 10);
        assert!(h.latest().is_none());
        for t in 0..25u8 {
            h.push(&[t, t, t]); // zero-padded to 4
        }
        assert_eq!(h.total(), 25);
        assert_eq!(h.first(), 15);
        assert!(h.get(14).is_none());
        assert_eq!(h.get(15), Some(&[15, 15, 15, 0][..]));
        assert_eq!(h.latest(), Some(&[24, 24, 24, 0][..]));
        assert!(h.get(25).is_none());
        assert_eq!(h.data.len(), 40);
        assert!(h.set(20, &[9, 9, 9, 9]));
        assert_eq!(h.get(20), Some(&[9, 9, 9, 9][..]));
        assert!(!h.set(3, &[1]));
        h.push_silence(3);
        assert_eq!(h.total(), 28);
        assert_eq!(h.latest(), Some(&[0, 0, 0, 0][..]));
        h.clear();
        assert!(h.is_empty());
        assert!(h.latest().is_none());
    }
}
