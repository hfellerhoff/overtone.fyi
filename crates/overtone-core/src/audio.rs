//! Bounded ring of raw mono audio: the source of truth the analysis is
//! derived from. Keeping it lets the engine re-analyse when the analysis
//! settings change instead of throwing the display away.

pub struct AudioRing {
    /// Storage indexed by `absolute_index % capacity`; allocated on first push.
    data: Vec<f32>,
    capacity: usize,
    /// Samples ever pushed; the next sample gets this index.
    total: u64,
    /// Oldest index that holds real audio (later than `total - capacity`
    /// when a large push skipped ahead).
    base: u64,
}

impl AudioRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::new(),
            capacity: capacity.max(1),
            total: 0,
            base: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    /// Oldest sample index still available.
    pub fn first(&self) -> u64 {
        self.total
            .saturating_sub(self.capacity as u64)
            .max(self.base)
    }

    pub fn len(&self) -> usize {
        (self.total - self.first()) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        if self.data.is_empty() {
            self.data = vec![0.0; self.capacity];
        }
        let src = if samples.len() > self.capacity {
            &samples[samples.len() - self.capacity..]
        } else {
            samples
        };
        // Skipped samples still advance time; they are simply unavailable.
        let skipped = (samples.len() - src.len()) as u64;
        if skipped > 0 {
            self.total += skipped;
            self.base = self.total;
        }
        let cap = self.capacity as u64;
        let mut pos = (self.total % cap) as usize;
        let first_run = src.len().min(self.capacity - pos);
        self.data[pos..pos + first_run].copy_from_slice(&src[..first_run]);
        if first_run < src.len() {
            self.data[..src.len() - first_run].copy_from_slice(&src[first_run..]);
        }
        pos += src.len();
        let _ = pos;
        self.total += src.len() as u64;
    }

    /// Copy samples `start..start + out.len()` into `out`. Samples before the
    /// ring's start or after its end are written as silence; returns whether
    /// any real samples were copied.
    pub fn read(&self, start: u64, out: &mut [f32]) -> bool {
        let mut any = false;
        let first = self.first();
        let cap = self.capacity as u64;
        for (i, o) in out.iter_mut().enumerate() {
            let idx = start + i as u64;
            *o = if idx >= first && idx < self.total {
                any = true;
                self.data[(idx % cap) as usize]
            } else {
                0.0
            };
        }
        any
    }

    /// A ring with a new capacity holding as much of the newest audio as
    /// fits, with the same absolute sample indices.
    pub fn resized(&self, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let keep = capacity.min(self.len());
        let mut out = Self::new(capacity);
        out.total = self.total - keep as u64;
        out.base = out.total;
        let mut buf = vec![0.0f32; keep];
        self.read(out.total, &mut buf);
        out.push(&buf);
        out
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.total = 0;
        self.base = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_samples_and_reads_by_absolute_index() {
        let mut r = AudioRing::new(8);
        r.push(&[1.0, 2.0, 3.0]);
        r.push(&[4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        assert_eq!(r.total(), 10);
        assert_eq!(r.first(), 2);
        assert_eq!(r.len(), 8);
        let mut out = [0.0; 4];
        assert!(r.read(2, &mut out));
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0]);
        assert!(r.read(8, &mut out));
        assert_eq!(out, [9.0, 10.0, 0.0, 0.0]);
        assert!(!r.read(0, &mut [0.0; 2]));
        let mut out = [0.0; 3];
        assert!(r.read(1, &mut out));
        assert_eq!(out, [0.0, 3.0, 4.0]);
        // a push larger than the ring keeps only its tail and counts the rest
        r.push(&(0..20).map(|i| i as f32).collect::<Vec<_>>());
        assert_eq!(r.total(), 30);
        let mut out = [0.0; 8];
        assert!(r.read(22, &mut out));
        assert_eq!(out, [12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0]);
        let small = r.resized(3);
        assert_eq!(small.total(), 30);
        assert_eq!(small.first(), 27);
        let mut out = [0.0; 3];
        assert!(small.read(27, &mut out));
        assert_eq!(out, [17.0, 18.0, 19.0]);
        let big = r.resized(100);
        assert_eq!(big.first(), 22);
        assert!(big.read(22, &mut [0.0; 1]));
        let mut out = [0.0; 2];
        assert!(big.read(28, &mut out));
        assert_eq!(out, [18.0, 19.0]);
        // filling past the wrap after a partial start
        let mut w = big;
        w.push(&vec![1.0; 200]);
        assert_eq!(w.first(), 130);
        assert!(w.read(130, &mut [0.0; 100]));
    }
}
