//! A faithful re-implementation of the Web Audio `AnalyserNode` frequency
//! analysis as implemented in Chromium (`RealtimeAnalyser`):
//!
//! * the most recent `fft_size` samples are taken from a ring buffer,
//! * a Blackman window is applied,
//! * a real FFT is computed and scaled so that a full-scale sine registers
//!   as 0 dBFS (`2 * |X[k]| / N`),
//! * magnitudes are converted to decibels and mapped linearly from
//!   `[min_decibels, max_decibels]` onto `[0, 255]`.

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Default `AnalyserNode.minDecibels`.
pub const MIN_DECIBELS: f32 = -100.0;
/// Default `AnalyserNode.maxDecibels`.
pub const MAX_DECIBELS: f32 = -30.0;

pub struct Analyser {
    fft_size: usize,
    ring: Vec<f32>,
    write_pos: usize,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    time_buf: Vec<f32>,
    freq_buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    magnitudes: Vec<f32>,
    bytes: Vec<u8>,
    smoothing: f32,
    min_db: f32,
    max_db: f32,
    dirty: bool,
    since_analysis: usize,
}

impl Analyser {
    /// `fft_size` must be a power of two >= 32 (same constraint as the Web Audio API).
    pub fn new(fft_size: usize) -> Self {
        assert!(
            fft_size >= 32 && fft_size.is_power_of_two(),
            "fft_size must be a power of two >= 32"
        );
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let scratch = fft.make_scratch_vec();
        let freq_buf = fft.make_output_vec();
        Self {
            fft_size,
            ring: vec![0.0; fft_size],
            write_pos: 0,
            window: blackman_window(fft_size),
            fft,
            time_buf: vec![0.0; fft_size],
            freq_buf,
            scratch,
            magnitudes: vec![0.0; fft_size / 2],
            bytes: vec![0; fft_size / 2],
            smoothing: 0.0,
            min_db: MIN_DECIBELS,
            max_db: MAX_DECIBELS,
            dirty: true,
            since_analysis: 0,
        }
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Number of frequency bins produced (`fft_size / 2`), like `frequencyBinCount`.
    pub fn bin_count(&self) -> usize {
        self.fft_size / 2
    }

    /// `AnalyserNode.smoothingTimeConstant` (the app uses 0).
    pub fn set_smoothing(&mut self, k: f32) {
        self.smoothing = k.clamp(0.0, 1.0);
    }

    pub fn set_decibel_range(&mut self, min_db: f32, max_db: f32) {
        self.min_db = min_db;
        self.max_db = max_db;
    }

    /// Reset the sample history (and the smoothed magnitudes) to silence.
    pub fn clear(&mut self) {
        self.ring.iter_mut().for_each(|s| *s = 0.0);
        self.magnitudes.iter_mut().for_each(|m| *m = 0.0);
        self.write_pos = 0;
        self.since_analysis = 0;
        self.dirty = true;
    }

    /// Append mono samples. Only the most recent `fft_size` samples are kept.
    pub fn push_mono(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let n = self.fft_size;
        let src = if samples.len() > n {
            &samples[samples.len() - n..]
        } else {
            samples
        };
        let first = src.len().min(n - self.write_pos);
        self.ring[self.write_pos..self.write_pos + first].copy_from_slice(&src[..first]);
        let rest = src.len() - first;
        if rest > 0 {
            self.ring[..rest].copy_from_slice(&src[first..]);
        }
        self.write_pos = (self.write_pos + src.len()) % n;
        self.since_analysis = self.since_analysis.saturating_add(samples.len());
        self.dirty = true;
    }

    /// Samples pushed since the last [`Self::analyse`].
    pub fn samples_since_analysis(&self) -> usize {
        self.since_analysis
    }

    /// Append interleaved multi-channel samples; channels are averaged into
    /// mono exactly like the Web Audio analyser down-mixes its input.
    pub fn push_interleaved(&mut self, data: &[f32], channels: usize) {
        if channels <= 1 {
            self.push_mono(data);
            return;
        }
        let scale = 1.0 / channels as f32;
        let mut mono = Vec::with_capacity(data.len() / channels);
        for frame in data.chunks_exact(channels) {
            mono.push(frame.iter().sum::<f32>() * scale);
        }
        self.push_mono(&mono);
    }

    /// True when samples arrived since the last [`Self::analyse`] call.
    pub fn has_new_samples(&self) -> bool {
        self.dirty
    }

    /// Recompute magnitudes from the current sample history if new samples
    /// arrived since the last analysis.
    pub fn analyse(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.since_analysis = 0;
        let n = self.fft_size;
        let wp = self.write_pos;
        // Unroll the ring so that time_buf holds the last n samples in order.
        self.time_buf[..n - wp].copy_from_slice(&self.ring[wp..]);
        self.time_buf[n - wp..].copy_from_slice(&self.ring[..wp]);
        for (s, w) in self.time_buf.iter_mut().zip(self.window.iter()) {
            *s *= *w;
        }
        self.fft
            .process_with_scratch(&mut self.time_buf, &mut self.freq_buf, &mut self.scratch)
            .expect("buffer sizes are fixed at construction");

        // Chromium: "Normalize so than an input sine wave at 0dBfs registers as 0dBfs".
        let magnitude_scale = 2.0 / n as f32;
        let k = self.smoothing;
        for (i, m) in self.magnitudes.iter_mut().enumerate() {
            let c = self.freq_buf[i];
            let mag = (c.re * c.re + c.im * c.im).sqrt() * magnitude_scale;
            *m = if k > 0.0 {
                k * *m + (1.0 - k) * mag
            } else {
                mag
            };
        }
    }

    /// Linear magnitudes for bins `0..fft_size/2` (after [`Self::analyse`]).
    pub fn magnitudes(&self) -> &[f32] {
        &self.magnitudes
    }

    /// The bytes computed by the last [`Self::byte_frequency_data`] call.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Equivalent of `AnalyserNode.getByteFrequencyData`.
    pub fn byte_frequency_data(&mut self) -> &[u8] {
        self.analyse();
        let range_scale = if self.max_db == self.min_db {
            1.0
        } else {
            1.0 / (self.max_db - self.min_db)
        };
        for (b, &m) in self.bytes.iter_mut().zip(self.magnitudes.iter()) {
            let db = linear_to_decibels(m);
            let scaled = 255.0 * (db - self.min_db) * range_scale;
            *b = scaled.clamp(0.0, 255.0) as u8;
        }
        &self.bytes
    }
}

/// Chromium's `audio_utilities::LinearToDecibels`.
#[inline]
pub fn linear_to_decibels(linear: f32) -> f32 {
    if linear > 0.0 {
        20.0 * linear.log10()
    } else {
        -1000.0
    }
}

/// Blackman window as used by Chromium's `RealtimeAnalyser::ApplyWindow`.
pub fn blackman_window(n: usize) -> Vec<f32> {
    let alpha = 0.16f64;
    let a0 = 0.5 * (1.0 - alpha);
    let a1 = 0.5;
    let a2 = 0.5 * alpha;
    (0..n)
        .map(|i| {
            let x = i as f64 / n as f64;
            (a0 - a1 * (2.0 * std::f64::consts::PI * x).cos()
                + a2 * (4.0 * std::f64::consts::PI * x).cos()) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize, freq: f32, sample_rate: f32, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn peak_bin_and_scaling_match_chromium() {
        let n = 8192;
        let sr = 48000.0;
        let bin = 100;
        let freq = bin as f32 * sr / n as f32;
        let mut a = Analyser::new(n);
        // amplitude 0.01 -> peak magnitude 0.01 * 0.42 (Blackman a0) = -47.5 dB
        a.push_mono(&sine(n, freq, sr, 0.01));
        let bytes = a.byte_frequency_data().to_vec();
        let (peak, &peak_val) = bytes.iter().enumerate().max_by_key(|(_, &v)| v).unwrap();
        assert_eq!(peak, bin);
        // 255 * (-47.535 + 100) / 70 = 191.1
        assert!((190..=192).contains(&peak_val), "peak byte {peak_val}");
        // far away bins are silent
        assert_eq!(bytes[2000], 0);
    }

    #[test]
    fn ring_keeps_most_recent_samples() {
        let mut a = Analyser::new(32);
        a.push_mono(&[1.0; 40]);
        a.push_mono(&[0.0; 16]);
        a.analyse();
        let n = 32;
        let wp = a.write_pos;
        let mut ordered = Vec::new();
        ordered.extend_from_slice(&a.ring[wp..]);
        ordered.extend_from_slice(&a.ring[..wp]);
        assert_eq!(&ordered[..16], &[1.0; 16]);
        assert_eq!(&ordered[16..], &[0.0; 16]);
        assert_eq!(n, ordered.len());
    }

    #[test]
    fn silence_is_zero() {
        let mut a = Analyser::new(1024);
        assert!(a.byte_frequency_data().iter().all(|&b| b == 0));
    }
}
