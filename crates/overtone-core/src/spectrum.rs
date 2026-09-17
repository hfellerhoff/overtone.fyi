//! Multi-resolution spectrum layout.
//!
//! The engine runs one FFT per band, each with a different window length:
//! long windows for the bass (fine pitch detail, slow) and short windows
//! for the treble (coarser pitch, fast). Each tick, the bins each band is
//! responsible for are copied into one composite byte spectrum whose layout
//! this module describes.

use serde::{Deserialize, Serialize};

/// Half-width of the crossfade zone at each band boundary, in octaves.
/// Rows within this distance of a boundary blend the two bands so the
/// change in window length does not show as a hard seam.
pub const TRANSITION_OCTAVES: f64 = 1.0 / 6.0;

/// `2 ^ TRANSITION_OCTAVES`.
pub fn transition_ratio() -> f64 {
    2f64.powf(TRANSITION_OCTAVES)
}

/// One frequency band and the window length it is analysed with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    /// Upper edge of the band in Hz; `None` extends to the Nyquist frequency.
    pub max_hz: Option<f64>,
    /// Window length = base FFT size / divisor (a power of two).
    pub divisor: usize,
}

/// The default layout: halve the window every octave above 250 Hz.
pub fn default_bands() -> Vec<Band> {
    vec![
        Band {
            max_hz: Some(250.0),
            divisor: 1,
        },
        Band {
            max_hz: Some(500.0),
            divisor: 2,
        },
        Band {
            max_hz: Some(1000.0),
            divisor: 4,
        },
        Band {
            max_hz: None,
            divisor: 8,
        },
    ]
}

/// A single band covering everything with the base window.
pub fn single_band() -> Vec<Band> {
    vec![Band {
        max_hz: None,
        divisor: 1,
    }]
}

/// Named relative layouts. Every band's window is a fixed fraction of the
/// base FFT size, so changing the base scales them all together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BandPreset {
    /// One window for the whole range.
    Single,
    /// Halve the window per octave above 250 Hz (four bands, down to 1/8).
    Balanced,
    /// Halve the window per octave above 125 Hz (six bands, down to 1/32).
    Sharp,
}

impl BandPreset {
    pub fn bands(self) -> Vec<Band> {
        match self {
            BandPreset::Single => single_band(),
            BandPreset::Balanced => default_bands(),
            BandPreset::Sharp => vec![
                Band {
                    max_hz: Some(125.0),
                    divisor: 1,
                },
                Band {
                    max_hz: Some(250.0),
                    divisor: 2,
                },
                Band {
                    max_hz: Some(500.0),
                    divisor: 4,
                },
                Band {
                    max_hz: Some(1000.0),
                    divisor: 8,
                },
                Band {
                    max_hz: Some(2000.0),
                    divisor: 16,
                },
                Band {
                    max_hz: None,
                    divisor: 32,
                },
            ],
        }
    }
}

pub fn validate_bands(bands: &[Band], base_fft: usize) -> Result<(), String> {
    if bands.is_empty() {
        return Err("at least one band is required".into());
    }
    let mut previous = 0.0;
    for (i, b) in bands.iter().enumerate() {
        if !b.divisor.is_power_of_two() || base_fft / b.divisor < 32 {
            return Err(format!("band {i}: invalid divisor {}", b.divisor));
        }
        match b.max_hz {
            Some(hz) if !(hz.is_finite() && hz > previous) => {
                return Err(format!("band {i}: maxHz {hz} must increase"));
            }
            Some(hz) => previous = hz,
            None if i + 1 != bands.len() => {
                return Err(format!("band {i}: only the last band may be open-ended"));
            }
            None => {}
        }
    }
    Ok(())
}

/// Where one band's bins live in the composite spectrum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub min_hz: f64,
    pub max_hz: f64,
    pub fft_size: usize,
    pub bin_hz: f64,
    /// First FFT bin of this band copied into the composite.
    pub first_bin: usize,
    /// Number of bins copied.
    pub bin_count: usize,
    /// Index of `first_bin` within the composite.
    pub offset: usize,
}

impl Segment {
    /// Composite index and fractional position of `hz` within this segment.
    pub fn position(&self, hz: f64) -> f64 {
        hz / self.bin_hz - self.first_bin as f64 + self.offset as f64
    }

    pub fn end(&self) -> usize {
        self.offset + self.bin_count
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub sample_rate: f64,
    pub base_fft: usize,
    pub segments: Vec<Segment>,
    /// Total composite length in bytes.
    pub len: usize,
}

impl Layout {
    pub fn build(sample_rate: f64, base_fft: usize, bands: &[Band]) -> Self {
        let nyquist = sample_rate / 2.0;
        let mut segments = Vec::with_capacity(bands.len());
        let mut lower = 0.0f64;
        let mut offset = 0usize;
        for band in bands {
            let upper = band.max_hz.map_or(nyquist, |hz| hz.min(nyquist));
            if upper <= lower {
                continue;
            }
            let fft_size = base_fft / band.divisor;
            let bin_hz = sample_rate / fft_size as f64;
            let bins = fft_size / 2;
            // Each band also carries the bins for the crossfade zones on
            // either side, plus one bin of margin for interpolation.
            let ratio = transition_ratio();
            let lower_ext = if lower > 0.0 { lower / ratio } else { 0.0 };
            let upper_ext = if band.max_hz.is_some() {
                upper * ratio
            } else {
                upper
            };
            let first_bin = ((lower_ext / bin_hz).floor() as usize).saturating_sub(1);
            let last_bin = ((upper_ext / bin_hz).ceil() as usize + 1).min(bins - 1);
            let bin_count = last_bin + 1 - first_bin;
            segments.push(Segment {
                min_hz: lower,
                max_hz: upper,
                fft_size,
                bin_hz,
                first_bin,
                bin_count,
                offset,
            });
            offset += bin_count;
            lower = upper;
        }
        Self {
            sample_rate,
            base_fft,
            segments,
            len: offset,
        }
    }

    pub fn single(sample_rate: f64, fft_size: usize) -> Self {
        Self::build(sample_rate, fft_size, &single_band())
    }

    /// The segment responsible for `hz`.
    pub fn segment_for(&self, hz: f64) -> Option<&Segment> {
        if hz < 0.0 {
            return None;
        }
        self.segments
            .iter()
            .find(|s| hz < s.max_hz)
            .or_else(|| self.segments.last().filter(|s| hz <= s.max_hz))
    }

    /// If `hz` lies in the crossfade zone between two bands, the lower and
    /// upper segments and the blend weight `t` (0 = all lower, 1 = all upper).
    pub fn blend_at(&self, hz: f64) -> Option<(&Segment, &Segment, f64)> {
        let ratio = transition_ratio();
        for pair in self.segments.windows(2) {
            let boundary = pair[0].max_hz;
            if hz > boundary / ratio && hz < boundary * ratio {
                let t = ((hz / boundary).ln() / ratio.ln() + 1.0) / 2.0;
                return Some((&pair[0], &pair[1], t.clamp(0.0, 1.0)));
            }
        }
        None
    }

    /// Distinct window lengths in use, largest first.
    pub fn fft_sizes(&self) -> Vec<usize> {
        let mut sizes: Vec<usize> = self.segments.iter().map(|s| s.fft_size).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        sizes.dedup();
        sizes
    }

    /// Copy each band's bins from its full spectrum into the composite.
    /// `spectra` yields the byte spectrum for a given window length.
    pub fn compose<'a>(&self, mut spectrum_for: impl FnMut(usize) -> &'a [u8], out: &mut [u8]) {
        debug_assert_eq!(out.len(), self.len);
        for s in &self.segments {
            let src = spectrum_for(s.fft_size);
            out[s.offset..s.end()].copy_from_slice(&src[s.first_bin..s.first_bin + s.bin_count]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_halves_the_window_per_octave() {
        let l = Layout::build(48000.0, 32768, &default_bands());
        assert_eq!(l.fft_sizes(), vec![32768, 16384, 8192, 4096]);
        assert_eq!(l.segments.len(), 4);
        assert_eq!(l.segments[0].min_hz, 0.0);
        assert_eq!(l.segments[0].max_hz, 250.0);
        assert_eq!(l.segments[3].min_hz, 1000.0);
        assert_eq!(l.segments[3].max_hz, 24000.0);
        // segments are contiguous in the composite
        for w in l.segments.windows(2) {
            assert_eq!(w[0].end(), w[1].offset);
        }
        assert_eq!(l.len, l.segments.last().unwrap().end());
        // far fewer bytes than the base window's full spectrum
        assert!(l.len < 3000, "{}", l.len);
        // bands overlap by the crossfade zone
        let s1 = l.segments[1];
        assert!(s1.first_bin as f64 * s1.bin_hz < 250.0 / transition_ratio());
        assert!((s1.first_bin + s1.bin_count) as f64 * s1.bin_hz > 500.0 * transition_ratio());
        // lookups
        assert_eq!(l.segment_for(100.0).unwrap().fft_size, 32768);
        assert_eq!(l.segment_for(250.0).unwrap().fft_size, 16384);
        assert_eq!(l.segment_for(5000.0).unwrap().fft_size, 4096);
        assert_eq!(l.segment_for(24000.0).unwrap().fft_size, 4096);
        assert!(l.segment_for(24001.0).is_none());
    }

    #[test]
    fn blend_zone_surrounds_each_boundary() {
        let l = Layout::build(48000.0, 32768, &default_bands());
        assert!(l.blend_at(200.0).is_none());
        assert!(l.blend_at(320.0).is_none());
        let (lo, hi, t) = l.blend_at(250.0).unwrap();
        assert_eq!((lo.fft_size, hi.fft_size), (32768, 16384));
        assert!((t - 0.5).abs() < 1e-9);
        let (_, _, t) = l.blend_at(230.0).unwrap();
        assert!(t > 0.1 && t < 0.2, "{t}");
        let (_, _, t) = l.blend_at(275.0).unwrap();
        assert!(t > 0.8 && t < 0.95, "{t}");
        let (lo, hi, _) = l.blend_at(1000.0).unwrap();
        assert_eq!((lo.fft_size, hi.fft_size), (8192, 4096));
    }

    #[test]
    fn positions_map_frequencies_into_the_composite() {
        let l = Layout::build(48000.0, 32768, &default_bands());
        let s = l.segment_for(300.0).unwrap();
        let p = s.position(300.0);
        assert!(p >= s.offset as f64 && p < s.end() as f64);
        // bin 300 / 2.93 = 102.4 relative to the band's own spectrum
        assert!((p - s.offset as f64 + s.first_bin as f64 - 102.4).abs() < 0.01);
    }

    #[test]
    fn compose_copies_each_bands_bins() {
        let l = Layout::build(
            48000.0,
            1024,
            &[
                Band {
                    max_hz: Some(6000.0),
                    divisor: 1,
                },
                Band {
                    max_hz: None,
                    divisor: 2,
                },
            ],
        );
        let big: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();
        let small: Vec<u8> = (0..256).map(|i| 200 - (i % 200) as u8).collect();
        let mut out = vec![0u8; l.len];
        l.compose(|n| if n == 1024 { &big } else { &small }, &mut out);
        let s0 = l.segments[0];
        let s1 = l.segments[1];
        assert_eq!(out[s0.offset], big[s0.first_bin]);
        assert_eq!(out[s1.offset], small[s1.first_bin]);
        assert_eq!(out[s1.end() - 1], small[s1.first_bin + s1.bin_count - 1]);
    }

    #[test]
    fn presets_are_valid_for_every_supported_base_size() {
        for preset in [BandPreset::Single, BandPreset::Balanced, BandPreset::Sharp] {
            for fft in [4096usize, 8192, 16384, 32768] {
                assert!(
                    validate_bands(&preset.bands(), fft).is_ok(),
                    "{preset:?} @ {fft}"
                );
            }
        }
        assert_eq!(BandPreset::Sharp.bands().last().unwrap().divisor, 32);
    }

    #[test]
    fn validation_rejects_bad_bands() {
        assert!(validate_bands(&default_bands(), 8192).is_ok());
        assert!(validate_bands(&[], 8192).is_err());
        assert!(validate_bands(
            &[Band {
                max_hz: None,
                divisor: 3
            }],
            8192
        )
        .is_err());
        assert!(validate_bands(
            &[Band {
                max_hz: None,
                divisor: 1024
            }],
            8192
        )
        .is_err());
        assert!(validate_bands(
            &[
                Band {
                    max_hz: None,
                    divisor: 1
                },
                Band {
                    max_hz: None,
                    divisor: 2
                }
            ],
            8192
        )
        .is_err());
        assert!(validate_bands(
            &[
                Band {
                    max_hz: Some(500.0),
                    divisor: 1
                },
                Band {
                    max_hz: Some(400.0),
                    divisor: 2
                }
            ],
            8192
        )
        .is_err());
        // bands above Nyquist are dropped rather than rejected
        let l = Layout::build(8000.0, 1024, &default_bands());
        assert_eq!(l.segments.len(), 4);
        assert_eq!(l.segments[3].max_hz, 4000.0);
    }
}
