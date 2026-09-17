//! Maps every output pixel row to a frequency and to the FFT bins it reads.
//!
//! Both scales place frequency logarithmically over the rows; they differ
//! only in the range they show by default. The view can be zoomed to any
//! sub-range (see [`FreqRange`]).

use crate::spectrum::{Layout, Segment};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    /// Piano range: 60 Hz up to just below C#9 (9.4 kHz).
    Piano,
    /// Full range above the piano floor: 60 Hz to 20 kHz (or Nyquist if lower).
    Logarithmic,
}

/// Lowest frequency shown by either scale.
pub const MIN_DISPLAY_HZ: f64 = 60.0;
/// Highest piano key shown (one above C8 + an octave, i.e. just below C#9).
pub const TOP_NOTE: f64 = 102.0;
pub const REFERENCE_HZ: f64 = 440.0;
pub const REFERENCE_NOTE_NUMBER: f64 = 49.0;
pub const LOG_MIN_HZ: f64 = MIN_DISPLAY_HZ;
pub const LOG_MAX_HZ: f64 = 20_000.0;
/// Hard limits for zooming and panning: nothing below the display floor or
/// above 20 kHz is ever shown.
pub const ABSOLUTE_MIN_HZ: f64 = MIN_DISPLAY_HZ;
pub const ABSOLUTE_MAX_HZ: f64 = LOG_MAX_HZ;
/// Smallest visible span, as a ratio max/min (a major third).
pub const MIN_SPAN_RATIO: f64 = 1.26;

pub fn semitone_factor() -> f64 {
    2f64.powf(1.0 / 12.0)
}

/// Equal-temperament frequency of piano key `pitch_number` (A4 = key 49).
pub fn pitch_by_number(pitch_number: f64) -> f64 {
    REFERENCE_HZ * semitone_factor().powf(pitch_number - REFERENCE_NOTE_NUMBER)
}

/// Fractional piano key number of `hz` (A4 = 49.0).
pub fn note_number(hz: f64) -> f64 {
    12.0 * (hz / REFERENCE_HZ).log2() + REFERENCE_NOTE_NUMBER
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreqRange {
    pub min_hz: f64,
    pub max_hz: f64,
}

impl FreqRange {
    pub fn new(min_hz: f64, max_hz: f64) -> Self {
        Self { min_hz, max_hz }
    }

    /// Default range for a scale, limited by the Nyquist frequency.
    pub fn preset(scale: Scale, sample_rate: f64) -> Self {
        let nyquist = sample_rate / 2.0;
        let (min, max) = match scale {
            Scale::Piano => (MIN_DISPLAY_HZ, pitch_by_number(TOP_NOTE)),
            Scale::Logarithmic => (LOG_MIN_HZ, LOG_MAX_HZ),
        };
        Self::new(min, max.min(nyquist))
    }

    /// Clamp to the hard limits, keeping the span at least [`MIN_SPAN_RATIO`].
    pub fn clamped(self, sample_rate: f64) -> Self {
        let nyquist = (sample_rate / 2.0).min(ABSOLUTE_MAX_HZ);
        let mut min = if self.min_hz.is_finite() {
            self.min_hz
        } else {
            ABSOLUTE_MIN_HZ
        };
        let mut max = if self.max_hz.is_finite() {
            self.max_hz
        } else {
            nyquist
        };
        min = min.clamp(ABSOLUTE_MIN_HZ, nyquist / MIN_SPAN_RATIO);
        max = max.clamp(ABSOLUTE_MIN_HZ * MIN_SPAN_RATIO, nyquist);
        if max / min < MIN_SPAN_RATIO {
            let centre = (min * max).sqrt();
            min = centre / MIN_SPAN_RATIO.sqrt();
            max = centre * MIN_SPAN_RATIO.sqrt();
            if min < ABSOLUTE_MIN_HZ {
                min = ABSOLUTE_MIN_HZ;
                max = min * MIN_SPAN_RATIO;
            }
            if max > nyquist {
                max = nyquist;
                min = max / MIN_SPAN_RATIO;
            }
        }
        Self::new(min, max)
    }

    pub fn log_span(&self) -> f64 {
        (self.max_hz / self.min_hz).ln()
    }

    /// Frequency at fraction `f` (0 = bottom, 1 = top) of the range.
    pub fn hz_at(&self, f: f64) -> f64 {
        self.min_hz * (self.log_span() * f).exp()
    }

    /// Fraction (0 = bottom, 1 = top) at which `hz` sits; may be outside 0..1.
    pub fn fraction_of(&self, hz: f64) -> f64 {
        (hz / self.min_hz).ln() / self.log_span()
    }
}

/// How a row reads the composite spectrum.
#[derive(Clone, Debug, PartialEq)]
pub enum RowBins {
    /// The row is narrower than a bin: interpolate between bin `k` and `k + 1`
    /// with weight `frac` on the upper bin.
    Interpolate { k: usize, frac: f32 },
    /// The row spans several bins: take the loudest of `lo..=hi`.
    Max { lo: usize, hi: usize },
    /// The row is in the crossfade zone between two bands: blend the lower
    /// band's reading with the upper band's, weight `t` on the upper.
    Blend {
        parts: Box<(RowBins, RowBins)>,
        t: f32,
    },
    /// Outside the spectrum.
    None,
}

impl RowBins {
    /// Read this row from a composite spectrum, 0..=255.
    #[inline]
    pub fn eval(&self, spectrum: &[u8]) -> f32 {
        match self {
            RowBins::Interpolate { k, frac } => {
                let a = spectrum.get(*k).copied().unwrap_or(0) as f32;
                let b = spectrum.get(k + 1).copied().unwrap_or(0) as f32;
                a + (b - a) * frac
            }
            RowBins::Max { lo, hi } => spectrum
                .get(*lo..=*hi)
                .map_or(0, |s| s.iter().copied().max().unwrap_or(0))
                as f32,
            RowBins::Blend { parts, t } => {
                let a = parts.0.eval(spectrum);
                let b = parts.1.eval(spectrum);
                a + (b - a) * t
            }
            RowBins::None => 0.0,
        }
    }
}

/// Bins of `seg` that a row spanning `lo_hz..hi_hz` (centre `centre`) reads.
fn bins_in(seg: &Segment, lo_hz: f64, hi_hz: f64, centre: f64) -> RowBins {
    // composite indices of the bins whose centre lies inside the row,
    // clamped to the segment this row belongs to
    let lo_bin = seg.position(lo_hz).ceil().max(seg.offset as f64) as usize;
    let hi_bin = (seg.position(hi_hz).ceil() as usize)
        .saturating_sub(1)
        .min(seg.end() - 1);
    if hi_bin > lo_bin {
        return RowBins::Max {
            lo: lo_bin,
            hi: hi_bin,
        };
    }
    let pos = seg.position(centre);
    if pos < seg.offset as f64 {
        return RowBins::None;
    }
    let k = pos.floor() as usize;
    if k + 1 < seg.end() {
        RowBins::Interpolate {
            k,
            frac: (pos - k as f64) as f32,
        }
    } else {
        RowBins::None
    }
}

#[derive(Clone, Debug, Default)]
pub struct RowMap {
    pub range: FreqRange,
    /// Centre frequency of each row (row 0 is the lowest frequency).
    pub hz: Vec<f64>,
    /// Lower edge frequency of each row, plus the top edge as the last entry.
    pub edges: Vec<f64>,
    pub bins: Vec<RowBins>,
}

impl RowMap {
    pub fn build(range: FreqRange, height: usize, layout: &Layout) -> Self {
        let edges: Vec<f64> = (0..=height)
            .map(|i| range.hz_at(i as f64 / height as f64))
            .collect();
        let mut hz = Vec::with_capacity(height);
        let mut bins = Vec::with_capacity(height);
        for i in 0..height {
            let lo_hz = edges[i];
            let hi_hz = edges[i + 1];
            let centre = (lo_hz * hi_hz).sqrt();
            hz.push(centre);
            let row = match layout.blend_at(centre) {
                Some((low, high, t)) => RowBins::Blend {
                    parts: Box::new((
                        bins_in(low, lo_hz, hi_hz, centre),
                        bins_in(high, lo_hz, hi_hz, centre),
                    )),
                    t: t as f32,
                },
                None => match layout.segment_for(centre) {
                    Some(seg) => bins_in(seg, lo_hz, hi_hz, centre),
                    None => RowBins::None,
                },
            };
            bins.push(row);
        }
        Self {
            range,
            hz,
            edges,
            bins,
        }
    }

    pub fn height(&self) -> usize {
        self.hz.len()
    }

    /// Value of one row from a composite byte spectrum, 0..=255.
    #[inline]
    pub fn value(&self, i: usize, spectrum: &[u8]) -> f32 {
        self.bins[i].eval(spectrum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_the_expected_ranges() {
        let piano = FreqRange::preset(Scale::Piano, 48000.0);
        assert_eq!(piano.min_hz, 60.0);
        assert!((piano.max_hz - 9397.27).abs() < 0.01);
        let log = FreqRange::preset(Scale::Logarithmic, 48000.0);
        assert_eq!(log, FreqRange::new(60.0, 20000.0));
        let log_low_rate = FreqRange::preset(Scale::Logarithmic, 16000.0);
        assert_eq!(log_low_rate.max_hz, 8000.0);
        assert!((pitch_by_number(49.0) - 440.0).abs() < 1e-9);
        assert!((note_number(880.0) - 61.0).abs() < 1e-9);
    }

    #[test]
    fn range_is_logarithmic_and_invertible() {
        let r = FreqRange::new(20.0, 20000.0);
        assert!((r.hz_at(0.0) - 20.0).abs() < 1e-9);
        assert!((r.hz_at(1.0) - 20000.0).abs() < 1e-6);
        assert!((r.hz_at(1.0 / 3.0) - 200.0).abs() < 1e-6);
        assert!((r.fraction_of(2000.0) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn clamping_respects_limits() {
        let r = FreqRange::new(1.0, 1e9).clamped(48000.0);
        assert_eq!(r, FreqRange::new(60.0, 20000.0));
        let r = FreqRange::new(1.0, 1e9).clamped(16000.0);
        assert_eq!(r, FreqRange::new(60.0, 8000.0));
        let r = FreqRange::new(440.0, 441.0).clamped(48000.0);
        assert!((r.max_hz / r.min_hz - MIN_SPAN_RATIO).abs() < 1e-9);
        assert!(r.min_hz < 440.0 && r.max_hz > 441.0);
        let r = FreqRange::new(f64::NAN, 100.0).clamped(48000.0);
        assert!(r.min_hz.is_finite());
    }

    #[test]
    fn rows_read_bins_at_their_own_frequency() {
        let map = RowMap::build(
            FreqRange::preset(Scale::Piano, 48000.0),
            1092,
            &Layout::single(48000.0, 8192),
        );
        let bin = 48000.0 / 8192.0;
        assert_eq!(map.height(), 1092);
        for i in 0..map.height() {
            match &map.bins[i] {
                RowBins::Interpolate { k, frac } => {
                    let pos = *k as f64 + *frac as f64;
                    assert!((pos * bin - map.hz[i]).abs() < 0.01, "row {i}");
                }
                RowBins::Max { lo, hi } => {
                    assert!(*lo as f64 * bin >= map.edges[i] - 1e-9, "row {i}");
                    assert!(*hi as f64 * bin < map.edges[i + 1] + 1e-9, "row {i}");
                }
                other => panic!("row {i}: unexpected {other:?}"),
            }
        }
        // low rows are narrower than a bin, high rows span several
        assert!(matches!(map.bins[0], RowBins::Interpolate { .. }));
        assert!(matches!(map.bins[1091], RowBins::Max { .. }));
        for w in map.hz.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn multi_resolution_rows_read_their_bands_segment() {
        let layout = Layout::build(48000.0, 32768, &crate::spectrum::balanced_bands());
        let map = RowMap::build(FreqRange::new(30.0, 20000.0), 800, &layout);
        fn span(b: &RowBins) -> (usize, usize) {
            match b {
                RowBins::Interpolate { k, .. } => (*k, k + 1),
                RowBins::Max { lo, hi } => (*lo, *hi),
                other => panic!("unexpected {other:?}"),
            }
        }
        let check = |seg: &Segment, b: &RowBins, hz: f64, i: usize| {
            let (lo, hi) = span(b);
            assert!(
                lo >= seg.offset && hi < seg.end(),
                "row {i} ({hz} Hz) outside its segment"
            );
            // the bin actually read sits at the row's frequency
            let hz_of = |idx: usize| (idx - seg.offset + seg.first_bin) as f64 * seg.bin_hz;
            assert!(
                hz_of(lo) <= hz + seg.bin_hz && hz_of(hi) >= hz - seg.bin_hz,
                "row {i}"
            );
        };
        let mut blended = 0;
        for i in 0..map.height() {
            let hz = map.hz[i];
            match &map.bins[i] {
                RowBins::Blend { parts, t } => {
                    let (low, high, expected_t) = layout.blend_at(hz).unwrap();
                    assert!((*t as f64 - expected_t).abs() < 1e-6);
                    check(low, &parts.0, hz, i);
                    check(high, &parts.1, hz, i);
                    blended += 1;
                }
                RowBins::None => panic!("row {i} unmapped"),
                other => check(layout.segment_for(hz).unwrap(), other, hz, i),
            }
        }
        // three boundaries, each blended over a third of an octave
        let rows_per_octave = 800.0 / (20000f64 / 30.0).log2();
        assert!(
            (blended as f64 - rows_per_octave).abs() < 6.0,
            "{blended} blended rows"
        );
    }

    #[test]
    fn blended_rows_crossfade_between_bands() {
        let layout = Layout::build(48000.0, 32768, &crate::spectrum::balanced_bands());
        // lower band reads 200 everywhere, upper band reads 100 everywhere
        let mut spectrum = vec![0u8; layout.len];
        for s in &layout.segments {
            let v = if s.fft_size == 32768 { 200 } else { 100 };
            spectrum[s.offset..s.end()].iter_mut().for_each(|b| *b = v);
        }
        let map = RowMap::build(FreqRange::new(200.0, 320.0), 300, &layout);
        let values: Vec<f32> = (0..300).map(|i| map.value(i, &spectrum)).collect();
        assert_eq!(values[0], 200.0);
        assert_eq!(values[299], 100.0);
        // monotone, gradual descent through the zone around 250 Hz
        for w in values.windows(2) {
            assert!(w[1] <= w[0] + 1e-3, "{w:?}");
            assert!(w[0] - w[1] < 6.0, "step too large: {w:?}");
        }
        let mid = map.hz.iter().position(|&h| h >= 250.0).unwrap();
        assert!((values[mid] - 150.0).abs() < 5.0, "{}", values[mid]);
    }

    #[test]
    fn wide_rows_take_the_loudest_bin() {
        let map = RowMap::build(
            FreqRange::new(1000.0, 20000.0),
            100,
            &Layout::single(48000.0, 1024),
        );
        let mut spectrum = vec![0u8; 512];
        let RowBins::Max { lo, hi } = map.bins[50].clone() else {
            panic!("expected a wide row");
        };
        spectrum[lo] = 10;
        spectrum[hi] = 200;
        assert_eq!(map.value(50, &spectrum), 200.0);
        let map = RowMap::build(
            FreqRange::new(100.0, 110.0),
            100,
            &Layout::single(48000.0, 8192),
        );
        assert!(matches!(map.bins[0], RowBins::Interpolate { .. }));
        let mut spectrum = vec![0u8; 4096];
        let RowBins::Interpolate { k, frac } = map.bins[0].clone() else {
            unreachable!()
        };
        spectrum[k] = 100;
        spectrum[k + 1] = 200;
        assert!((map.value(0, &spectrum) - (100.0 + 100.0 * frac)).abs() < 1e-3);
    }
}
