//! The analysis engine: owns the analyser and all derived lookup tables and
//! turns the current audio into one binary frame packet (see [`crate::packet`]).

use crate::analyser::Analyser;
use crate::color::{build_lut, lut_index, Coloring};
use crate::labels::{frequency_markers, render_label_strip, Labeling, Marker};
use crate::mapping::{FreqRange, RowMap, Scale};
use crate::packet::{self, Header, FLAG_NEW_AUDIO};
use crate::pitch::{
    detect_pitch, overtone_buckets, pitch_table, spectral_peaks, Bucket, Pitch, PitchResult,
};
use serde::{Deserialize, Serialize};

/// Width of the frequency label strip in pixels.
pub const LABEL_WIDTH: usize = 64;
/// Width of the live spectrum panel in pixels (256 + 16).
pub const LIVE_WIDTH: usize = 272;
/// Minimum spacing between frequency markers, as a fraction of the height.
pub const MARKER_MIN_GAP: f64 = 0.028;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    pub fft_size: usize,
    pub sample_rate: f64,
    /// Number of rows (pixels) of the analysis canvases.
    pub height: usize,
    pub scale: Scale,
    pub coloring: Coloring,
    pub labeling: Labeling,
    /// Visible frequency range; `None` uses the scale's preset.
    #[serde(default)]
    pub range: Option<FreqRange>,
}

impl EngineConfig {
    /// The range actually shown, clamped to sane limits.
    pub fn effective_range(&self) -> FreqRange {
        self.range
            .unwrap_or_else(|| FreqRange::preset(self.scale, self.sample_rate))
            .clamped(self.sample_rate)
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fft_size: 8192,
            sample_rate: 48000.0,
            height: 546,
            scale: Scale::Piano,
            coloring: Coloring::Detailed,
            labeling: Labeling::Piano,
            range: None,
        }
    }
}

impl EngineConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(self.fft_size >= 32 && self.fft_size.is_power_of_two() && self.fft_size <= 1 << 16) {
            return Err(format!("invalid fftSize {}", self.fft_size));
        }
        if !(self.sample_rate.is_finite() && self.sample_rate > 0.0) {
            return Err(format!("invalid sampleRate {}", self.sample_rate));
        }
        if self.height < 2 || self.height > 8192 {
            return Err(format!("invalid height {}", self.height));
        }
        Ok(())
    }
}

/// Static information about the current configuration, sent to the frontend
/// once per (re)configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub height: usize,
    pub fft_size: usize,
    pub sample_rate: f64,
    pub label_width: usize,
    pub live_width: usize,
    /// Visible frequency range (bottom row to top row).
    pub range: FreqRange,
    /// Frequency markers, highest first.
    pub markers: Vec<Marker>,
    /// Note labels indexed by the packet's `note` field.
    pub notes: Vec<String>,
}

pub struct Engine {
    config: EngineConfig,
    analyser: Analyser,
    rows: RowMap,
    lut: Vec<[u8; 3]>,
    pitches: Vec<Pitch>,
    label_strip: Vec<u8>,
    packet: Vec<u8>,
    values: Vec<f32>,
    peaks: Vec<(f64, f64)>,
    buckets: Vec<Bucket>,
    seq: u32,
    last_pitch: PitchResult,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        config.validate()?;
        let analyser = Analyser::new(config.fft_size);
        let rows = RowMap::build(
            config.effective_range(),
            config.height,
            config.sample_rate,
            config.fft_size,
        );
        let pitches = pitch_table();
        let mut label_strip = Vec::new();
        render_label_strip(&rows, config.labeling, LABEL_WIDTH, &mut label_strip);
        let lut = build_lut(config.coloring);
        let height = config.height;
        Ok(Self {
            config,
            analyser,
            rows,
            lut,
            pitches,
            label_strip,
            packet: vec![0; packet::packet_len(height)],
            values: vec![0.0; height],
            peaks: Vec::with_capacity(256),
            buckets: Vec::with_capacity(256),
            seq: 0,
            last_pitch: PitchResult::default(),
        })
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Apply a new configuration, rebuilding only what changed. Audio history
    /// is kept unless the FFT size changes.
    pub fn configure(&mut self, config: EngineConfig) -> Result<(), String> {
        config.validate()?;
        let old = std::mem::replace(&mut self.config, config);
        let c = &self.config;
        if c.fft_size != old.fft_size {
            self.analyser = Analyser::new(c.fft_size);
        }
        let rows_changed = c.fft_size != old.fft_size
            || c.sample_rate != old.sample_rate
            || c.height != old.height
            || c.effective_range() != old.effective_range();
        if rows_changed {
            self.rows = RowMap::build(c.effective_range(), c.height, c.sample_rate, c.fft_size);
        }
        if rows_changed || c.labeling != old.labeling {
            render_label_strip(&self.rows, c.labeling, LABEL_WIDTH, &mut self.label_strip);
        }
        if c.coloring != old.coloring {
            self.lut = build_lut(c.coloring);
        }
        if c.height != old.height {
            self.packet = vec![0; packet::packet_len(c.height)];
            self.values = vec![0.0; c.height];
        }
        Ok(())
    }

    pub fn info(&self) -> EngineInfo {
        let markers = frequency_markers(&self.rows, MARKER_MIN_GAP);
        EngineInfo {
            height: self.config.height,
            fft_size: self.config.fft_size,
            sample_rate: self.config.sample_rate,
            label_width: LABEL_WIDTH,
            live_width: LIVE_WIDTH,
            range: self.rows.range,
            markers,
            notes: self.pitches.iter().map(|p| p.label.clone()).collect(),
        }
    }

    pub fn info_json(&self) -> String {
        serde_json::to_string(&self.info()).expect("info is serialisable")
    }

    /// RGBA label strip, `LABEL_WIDTH × height`, row 0 = top.
    pub fn label_strip(&self) -> &[u8] {
        &self.label_strip
    }

    pub fn push_mono(&mut self, samples: &[f32]) {
        self.analyser.push_mono(samples);
    }

    pub fn push_interleaved(&mut self, samples: &[f32], channels: usize) {
        self.analyser.push_interleaved(samples, channels);
    }

    /// Forget all audio history.
    pub fn clear(&mut self) {
        self.analyser.clear();
    }

    pub fn last_pitch(&self) -> PitchResult {
        self.last_pitch
    }

    /// Produce a frame from the audio that has been pushed so far.
    pub fn frame(&mut self) -> &[u8] {
        let fresh = self.analyser.has_new_samples();
        let bytes = self.analyser.byte_frequency_data();
        let flags = if fresh { FLAG_NEW_AUDIO } else { 0 };
        let pitch = render_frame(
            bytes,
            flags,
            &self.config,
            &self.rows,
            &self.lut,
            &self.pitches,
            &mut self.values,
            &mut self.peaks,
            &mut self.buckets,
            &mut self.seq,
            &mut self.packet,
        );
        self.last_pitch = pitch;
        &self.packet
    }

    /// Produce a frame from externally computed byte frequency data (e.g. a
    /// browser `AnalyserNode.getByteFrequencyData` result), bypassing the
    /// built-in FFT.
    pub fn frame_from_bytes(&mut self, bytes: &[u8]) -> &[u8] {
        let pitch = render_frame(
            bytes,
            FLAG_NEW_AUDIO,
            &self.config,
            &self.rows,
            &self.lut,
            &self.pitches,
            &mut self.values,
            &mut self.peaks,
            &mut self.buckets,
            &mut self.seq,
            &mut self.packet,
        );
        self.last_pitch = pitch;
        &self.packet
    }

    /// Row amplitudes (index 0 = lowest frequency) from the last frame.
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// Length in bytes of a frame packet for a canvas of `height` rows.
pub const fn packet_len_for(height: usize) -> usize {
    packet::packet_len(height)
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    bytes: &[u8],
    flags: u32,
    config: &EngineConfig,
    rows: &RowMap,
    lut: &[[u8; 3]],
    pitches: &[Pitch],
    values: &mut [f32],
    peaks: &mut Vec<(f64, f64)>,
    buckets: &mut Vec<Bucket>,
    seq: &mut u32,
    packet: &mut [u8],
) -> PitchResult {
    let height = config.height;

    // 1. row amplitudes
    for (i, v) in values.iter_mut().enumerate() {
        *v = rows.value(i, bytes);
    }

    // 2. spectral peaks within the scale's range → overtone buckets → pitch.
    //    The preset (not the zoomed) range is used so zooming the display
    //    does not change what the pitch detector listens to.
    let bin_hz = crate::mapping::bin_hz(config.sample_rate, config.fft_size);
    let detect = FreqRange::preset(config.scale, config.sample_rate);
    spectral_peaks(bytes, bin_hz, detect.min_hz, detect.max_hz, peaks);
    overtone_buckets(peaks, buckets);
    let pitch = detect_pitch(pitches, buckets);

    // 3. packet
    *seq = seq.wrapping_add(1);
    Header {
        seq: *seq,
        height: height as u32,
        flags,
        pitch_hz: pitch.hz as f32,
        target_hz: pitch.target_hz as f32,
        note: pitch.note.map_or(-1, |n| n as i32),
        sample_rate: config.sample_rate as f32,
    }
    .write(packet);

    let (column, live) = packet[packet::column_offset()..].split_at_mut(height * 4);
    // canvas row y (0 = top) shows analysis row i = height - 1 - y
    for y in 0..height {
        let v = values[height - 1 - y];
        let px = &mut column[y * 4..y * 4 + 4];
        if v > 0.0 {
            px[..3].copy_from_slice(&lut[lut_index(v)]);
        } else {
            px[..3].copy_from_slice(&[0, 0, 0]);
        }
        px[3] = 255;
        live[y * 4..y * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    pitch
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
    fn packet_has_expected_layout() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        let p = e.frame().to_vec();
        assert_eq!(p.len(), packet::packet_len(546));
        assert_eq!(
            u32::from_le_bytes(p[0..4].try_into().unwrap()),
            packet::MAGIC
        );
        assert_eq!(u32::from_le_bytes(p[8..12].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(p[12..16].try_into().unwrap()), 546);
        assert_eq!(i32::from_le_bytes(p[28..32].try_into().unwrap()), -1);
        // silence: black column, zero bars
        assert!(p[packet::column_offset()..packet::live_offset(546)]
            .chunks_exact(4)
            .all(|px| px == [0, 0, 0, 255]));
        assert!(p[packet::live_offset(546)..].iter().all(|&b| b == 0));
    }

    #[test]
    fn tone_lights_up_the_expected_row_and_pitch() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        e.push_mono(&sine(8192, 440.0, 48000.0, 0.5));
        let p = e.frame().to_vec();
        let pitch_hz = f32::from_le_bytes(p[20..24].try_into().unwrap());
        let note = i32::from_le_bytes(p[28..32].try_into().unwrap());
        assert!((pitch_hz - 440.0).abs() < 1.0, "pitch {pitch_hz}");
        assert_eq!(e.info().notes[note as usize], "A4");
        // brightest live bar must be near the 440 Hz row
        let values = e.values();
        let (row, _) = values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        let hz = e.rows.hz[row];
        assert!((hz - 440.0).abs() < 6.0, "row hz {hz}");
        assert_eq!(p[16..20], FLAG_NEW_AUDIO.to_le_bytes());
    }

    #[test]
    fn harmonic_tone_reports_its_fundamental() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        // A2 with a strong octave and two more partials, like a sung note.
        let sr = 48000.0;
        let mut samples = vec![0.0f32; 8192];
        for (k, amp) in [(1.0, 0.03), (2.0, 0.03), (3.0, 0.02), (4.0, 0.015)] {
            for (s, v) in samples.iter_mut().zip(sine(8192, 110.0 * k, sr, amp)) {
                *s += v;
            }
        }
        e.push_mono(&samples);
        let p = e.frame().to_vec();
        let pitch_hz = f32::from_le_bytes(p[20..24].try_into().unwrap());
        let note = i32::from_le_bytes(p[28..32].try_into().unwrap());
        assert!((pitch_hz - 110.0).abs() < 1.0, "pitch {pitch_hz}");
        assert_eq!(e.info().notes[note as usize], "A2");
    }

    #[test]
    fn log_scale_stops_at_20k_and_zoom_range_is_honoured() {
        let mut e = Engine::new(EngineConfig {
            scale: Scale::Logarithmic,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(e.info().range, FreqRange::new(20.0, 20000.0));
        let mut c = e.config().clone();
        c.range = Some(FreqRange::new(400.0, 800.0));
        e.configure(c).unwrap();
        let info = e.info();
        assert_eq!(info.range, FreqRange::new(400.0, 800.0));
        assert!((e.rows.hz[0] - 400.0).abs() < 1.0);
        // pitch detection still listens to the whole preset range
        e.push_mono(&sine(8192, 110.0, 48000.0, 0.05));
        let p = e.frame().to_vec();
        let pitch_hz = f32::from_le_bytes(p[20..24].try_into().unwrap());
        assert!((pitch_hz - 110.0).abs() < 1.0, "pitch {pitch_hz}");
    }

    #[test]
    fn reconfigure_updates_derived_tables() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        let strip_before = e.label_strip().to_vec();
        let mut c = e.config().clone();
        c.labeling = Labeling::Linear;
        c.height = 1092;
        c.fft_size = 16384;
        e.configure(c).unwrap();
        assert_ne!(e.label_strip(), &strip_before[..]);
        assert_eq!(e.label_strip().len(), LABEL_WIDTH * 1092 * 4);
        assert_eq!(e.frame().len(), packet::packet_len(1092));
        let info = e.info();
        assert!(info.markers.len() > 10);
        assert!(info.markers[0].fraction > info.markers[1].fraction);
        assert!((info.range.max_hz - 9397.27).abs() < 0.01);
        assert!(e
            .configure(EngineConfig {
                fft_size: 1000,
                ..Default::default()
            })
            .is_err());
    }
}
