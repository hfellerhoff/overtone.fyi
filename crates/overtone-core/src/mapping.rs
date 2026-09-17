//! Maps every output pixel row to a start frequency and to the FFT bins it
//! reads from. This is a direct port of `getHzDataArray` from the original
//! web implementation, so the on-screen result is unchanged.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    Piano,
    Logarithmic,
}

/// Number of piano notes spread across the canvas height.
pub const NOTES: f64 = 96.0;
/// The note number shown at the bottom row of the canvas.
pub const BASE_NOTE: f64 = 6.0;
pub const REFERENCE_HZ: f64 = 440.0;
pub const REFERENCE_NOTE_NUMBER: f64 = 49.0;

pub fn semitone_factor() -> f64 {
    2f64.powf(1.0 / 12.0)
}

/// Equal-temperament frequency of piano key `pitch_number` (A4 = key 49).
pub fn pitch_by_number(pitch_number: f64) -> f64 {
    REFERENCE_HZ * semitone_factor().powf(pitch_number - REFERENCE_NOTE_NUMBER)
}

pub fn piano_pixel_start(pixel_index: usize, pixel_count: usize) -> f64 {
    let note_pixel_distance = pixel_count as f64 / NOTES;
    let adjusted_pixel_index = BASE_NOTE * note_pixel_distance + pixel_index as f64;
    pitch_by_number(adjusted_pixel_index / note_pixel_distance)
}

pub fn logarithmic_pixel_start(max_hz: f64, pixel_count: usize, pixel_number: usize) -> f64 {
    let p = pixel_number as f64;
    (pixel_count as f64 / max_hz) * (p * p)
}

pub fn pixel_start(scale: Scale, max_hz: f64, height: usize, i: usize) -> f64 {
    match scale {
        Scale::Piano => piano_pixel_start(i, height),
        Scale::Logarithmic => logarithmic_pixel_start(max_hz, height, i),
    }
}

/// Width of one FFT bin in Hz. (The original web app divided by
/// `fftSize * 2`, which read every row one octave too high.)
pub fn bin_hz(sample_rate: f64, fft_size: usize) -> f64 {
    sample_rate / fft_size as f64
}

/// Terminal value `j` of the original nearest-bin search loop:
///
/// ```js
/// let j = 0, previousDistance = 1e8;
/// while (previousDistance > Math.abs(mid - j * binSizeHz)) {
///   previousDistance = Math.abs(mid - j * binSizeHz);
///   j++;
/// }
/// ```
///
/// The distance is strictly decreasing while `j * bin_hz <= mid`, so we can
/// start the literal loop two bins below the floor and get the same result
/// without walking from zero.
pub fn legacy_nearest_bin(mid: f64, bin_hz: f64) -> usize {
    let dist = |j: usize| (mid - j as f64 * bin_hz).abs();
    let floor = if bin_hz > 0.0 && mid.is_finite() {
        (mid / bin_hz).floor().max(0.0) as usize
    } else {
        0
    };
    let mut j = floor.saturating_sub(1);
    let mut previous = if j == 0 { 100_000_000.0 } else { dist(j - 1) };
    while previous > dist(j) {
        previous = dist(j);
        j += 1;
    }
    j
}

/// Literal port of the search loop, used to validate [`legacy_nearest_bin`].
#[cfg(test)]
fn literal_nearest_bin(mid: f64, bin_hz: f64) -> usize {
    let mut j = 0usize;
    let mut previous = 100_000_000.0f64;
    while previous > (mid - j as f64 * bin_hz).abs() {
        previous = (mid - j as f64 * bin_hz).abs();
        j += 1;
    }
    j
}

#[derive(Clone, Debug, Default)]
pub struct RowMap {
    /// Start frequency of each row (row 0 is the lowest frequency).
    pub start_hz: Vec<f64>,
    /// For each row, the lower of the two bins that are averaged
    /// (`j - 1` in the original), or `None` when the row is out of range.
    pub bins: Vec<Option<usize>>,
}

impl RowMap {
    pub fn build(scale: Scale, height: usize, sample_rate: f64, fft_size: usize) -> Self {
        let max_hz = sample_rate / 2.0;
        let bin_hz = bin_hz(sample_rate, fft_size);
        let bin_count = fft_size / 2;
        let mut start_hz = Vec::with_capacity(height);
        let mut bins = Vec::with_capacity(height);
        for i in 0..height {
            let start = pixel_start(scale, max_hz, height, i);
            let next = pixel_start(scale, max_hz, height, i + 1);
            let size = next - start;
            let mid = start + size / 2.0;
            let j = legacy_nearest_bin(mid, bin_hz);
            start_hz.push(start);
            bins.push(if j < bin_count { Some(j - 1) } else { None });
        }
        Self { start_hz, bins }
    }

    pub fn height(&self) -> usize {
        self.start_hz.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piano_scale_starts_at_base_note() {
        assert_eq!(piano_pixel_start(0, 1092), pitch_by_number(BASE_NOTE));
        assert!((pitch_by_number(49.0) - 440.0).abs() < 1e-9);
    }

    #[test]
    fn fast_nearest_bin_matches_literal_loop() {
        let bin_hz = 48000.0 / 16384.0;
        for scale in [Scale::Piano, Scale::Logarithmic] {
            for height in [546usize, 1092, 819] {
                for i in 0..height {
                    let start = pixel_start(scale, 24000.0, height, i);
                    let next = pixel_start(scale, 24000.0, height, i + 1);
                    let mid = start + (next - start) / 2.0;
                    assert_eq!(
                        legacy_nearest_bin(mid, bin_hz),
                        literal_nearest_bin(mid, bin_hz),
                        "scale {scale:?} height {height} row {i}"
                    );
                }
            }
        }
        for mid in [0.0, 0.5, 1.4648, 2.93, 12000.0, 54157.9] {
            assert_eq!(
                legacy_nearest_bin(mid, bin_hz),
                literal_nearest_bin(mid, bin_hz)
            );
        }
    }

    #[test]
    fn rows_read_the_bins_at_their_own_frequency() {
        let map = RowMap::build(Scale::Piano, 1092, 48000.0, 8192);
        let hz = 48000.0 / 8192.0;
        for i in 0..map.height() {
            let next = pixel_start(Scale::Piano, 24000.0, 1092, i + 1);
            let mid = (map.start_hz[i] + next) / 2.0;
            // The two averaged bins are the nearest bin and the one above it,
            // so the row's midpoint is within half a bin of the lower one.
            let lower = map.bins[i].unwrap() as f64 * hz;
            assert!(
                (mid - lower).abs() <= hz / 2.0 + 1e-9,
                "row {i}: mid {mid} vs bin {lower}"
            );
        }
    }

    #[test]
    fn rows_are_monotonic_and_in_range() {
        let map = RowMap::build(Scale::Piano, 1092, 48000.0, 8192);
        assert_eq!(map.height(), 1092);
        for w in map.start_hz.windows(2) {
            assert!(w[0] < w[1]);
        }
        assert!(map.bins.iter().all(|b| b.is_some()));
        let log = RowMap::build(Scale::Logarithmic, 1092, 48000.0, 8192);
        // the top of the "logarithmic" scale is beyond the array, as before
        assert!(log.bins.last().unwrap().is_none());
    }
}
