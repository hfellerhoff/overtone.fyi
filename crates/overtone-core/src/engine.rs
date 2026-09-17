//! The analysis engine: owns the analyser, the recorded spectrum history and
//! all derived lookup tables, and renders the requested view of the timeline
//! into one binary frame packet (see [`crate::packet`]).
//!
//! Time is measured in audio samples. Analysis runs every [`hop`] samples,
//! independent of how often frames are requested, and each tick's spectrum is
//! kept in a [`History`] ring so the display can scroll back in time and be
//! re-rendered at a different frequency range.

use crate::analyser::Analyser;
use crate::color::{build_lut, lut_index, Coloring};
use crate::history::History;
use crate::labels::{frequency_markers, render_label_strip, Labeling, Marker};
use crate::mapping::{FreqRange, RowMap, Scale};
use crate::packet::{self, Header, FLAG_FULL, FLAG_LIVE, FLAG_NEW_AUDIO};
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
/// Default memory budget for the spectrum history.
pub const DEFAULT_HISTORY_BYTES: usize = 256 << 20;
/// Audio that arrives while no frames are requested is dropped beyond this
/// many seconds, so a hidden window does not grow memory without bound.
pub const MAX_PENDING_SECONDS: f64 = 8.0;
/// Ticks folded into one timeline column are capped here when zoomed far out.
pub const MAX_TICKS_PER_COLUMN: usize = 64;

/// Samples between analysis ticks. Scaling with the FFT size keeps the
/// history data rate constant (16 bytes per sample) and the tick interval
/// well below the window length.
pub const fn hop(fft_size: usize) -> usize {
    fft_size / 32
}

fn default_history_bytes() -> usize {
    DEFAULT_HISTORY_BYTES
}

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
    /// Memory budget for recorded spectra.
    #[serde(default = "default_history_bytes")]
    pub history_bytes: usize,
}

impl EngineConfig {
    /// The range actually shown, clamped to sane limits.
    pub fn effective_range(&self) -> FreqRange {
        self.range
            .unwrap_or_else(|| FreqRange::preset(self.scale, self.sample_rate))
            .clamped(self.sample_rate)
    }

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

    /// Analysis ticks per second.
    pub fn ticks_per_second(&self) -> f64 {
        self.sample_rate / hop(self.fft_size) as f64
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
            history_bytes: DEFAULT_HISTORY_BYTES,
        }
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
    pub ticks_per_second: f64,
    /// Seconds of history the budget can hold.
    pub history_seconds: f64,
}

/// What the frontend wants to see this frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRequest {
    /// Timeline width in pixels.
    pub width: usize,
    pub px_per_second: f64,
    /// Time (seconds) at the newest edge of the timeline; `None` follows live.
    #[serde(default)]
    pub view_end: Option<f64>,
}

/// The view the frontend currently has on its canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SentView {
    width: usize,
    height: usize,
    px_per_second: f64,
    /// Newest edge in whole pixels: `view_end = end_px / px_per_second`.
    end_px: i64,
    rows_version: u64,
}

pub struct Engine {
    config: EngineConfig,
    analyser: Analyser,
    history: History,
    pending: Vec<f32>,
    rows: RowMap,
    rows_version: u64,
    lut: Vec<[u8; 3]>,
    pitches: Vec<Pitch>,
    label_strip: Vec<u8>,
    packet: Vec<u8>,
    values: Vec<f32>,
    column: Vec<f32>,
    peaks: Vec<(f64, f64)>,
    buckets: Vec<Bucket>,
    seq: u32,
    last_pitch: PitchResult,
    sent: Option<SentView>,
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
        let history = History::new(config.fft_size / 2, config.history_bytes);
        Ok(Self {
            config,
            analyser,
            history,
            pending: Vec::new(),
            rows,
            rows_version: 0,
            lut,
            pitches,
            label_strip,
            packet: Vec::new(),
            values: vec![0.0; height],
            column: vec![0.0; height],
            peaks: Vec::with_capacity(256),
            buckets: Vec::with_capacity(256),
            seq: 0,
            last_pitch: PitchResult::default(),
            sent: None,
        })
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Apply a new configuration, rebuilding only what changed. Recorded
    /// history is kept unless the FFT size or sample rate changes.
    pub fn configure(&mut self, config: EngineConfig) -> Result<(), String> {
        config.validate()?;
        let old = std::mem::replace(&mut self.config, config);
        let c = &self.config;
        if c.fft_size != old.fft_size {
            self.analyser = Analyser::new(c.fft_size);
        }
        if c.fft_size != old.fft_size
            || c.sample_rate != old.sample_rate
            || c.history_bytes != old.history_bytes
        {
            self.history = History::new(c.fft_size / 2, c.history_bytes);
            self.pending.clear();
            self.sent = None;
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
        if rows_changed || c.coloring != old.coloring {
            self.rows_version += 1;
        }
        if c.height != old.height {
            self.values = vec![0.0; c.height];
            self.column = vec![0.0; c.height];
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
            ticks_per_second: self.config.ticks_per_second(),
            history_seconds: self.history.capacity() as f64 / self.config.ticks_per_second(),
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
        self.pending.extend_from_slice(samples);
        let max_pending = (self.config.sample_rate * MAX_PENDING_SECONDS) as usize;
        if self.pending.len() > max_pending {
            let excess = self.pending.len() - max_pending;
            self.pending.drain(..excess);
        }
    }

    pub fn push_interleaved(&mut self, samples: &[f32], channels: usize) {
        if channels <= 1 {
            self.push_mono(samples);
            return;
        }
        let scale = 1.0 / channels as f32;
        let mono: Vec<f32> = samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() * scale)
            .collect();
        self.push_mono(&mono);
    }

    /// Forget all audio and recorded history.
    pub fn clear(&mut self) {
        self.analyser.clear();
        self.history.clear();
        self.pending.clear();
        self.sent = None;
    }

    pub fn last_pitch(&self) -> PitchResult {
        self.last_pitch
    }

    /// Run analysis on all complete hops of pending audio. Returns how many
    /// ticks were recorded.
    pub fn process_pending(&mut self) -> usize {
        let hop = hop(self.config.fft_size);
        let mut ticks = 0;
        let mut offset = 0;
        while offset + hop <= self.pending.len() {
            self.analyser.push_mono(&self.pending[offset..offset + hop]);
            self.history.push(self.analyser.byte_frequency_data());
            offset += hop;
            ticks += 1;
        }
        if offset > 0 {
            self.pending.drain(..offset);
        }
        ticks
    }

    /// Oldest and newest recorded time in seconds.
    pub fn history_span(&self) -> (f64, f64) {
        let tps = self.config.ticks_per_second();
        (
            self.history.first() as f64 / tps,
            self.history.total() as f64 / tps,
        )
    }

    /// Analyse pending audio and render the requested view into a packet.
    pub fn frame(&mut self, view: ViewRequest) -> &[u8] {
        let fresh = self.process_pending() > 0;
        let height = self.config.height;
        let tps = self.config.ticks_per_second();
        let pps = if view.px_per_second.is_finite() && view.px_per_second > 0.0 {
            view.px_per_second
        } else {
            60.0
        };
        let width = view.width.clamp(1, 1 << 14);

        // Latest spectrum feeds the live bars and the pitch detector.
        let pitch = match self.history.latest() {
            Some(spectrum) => {
                for (i, v) in self.values.iter_mut().enumerate() {
                    *v = self.rows.value(i, spectrum);
                }
                let bin_hz = crate::mapping::bin_hz(self.config.sample_rate, self.config.fft_size);
                // The preset (not the zoomed) range is used so zooming the
                // display does not change what the pitch detector hears.
                let detect = FreqRange::preset(self.config.scale, self.config.sample_rate);
                spectral_peaks(
                    spectrum,
                    bin_hz,
                    detect.min_hz,
                    detect.max_hz,
                    &mut self.peaks,
                );
                overtone_buckets(&self.peaks, &mut self.buckets);
                detect_pitch(&self.pitches, &self.buckets)
            }
            None => {
                self.values.iter_mut().for_each(|v| *v = 0.0);
                PitchResult::default()
            }
        };
        self.last_pitch = pitch;

        // Resolve the requested view to a whole-pixel end position.
        let (history_start, history_end) = self.history_span();
        let following = view.view_end.is_none_or(|t| t >= history_end);
        let target_end = view
            .view_end
            .map_or(history_end, |t| t.clamp(history_start, history_end));
        let end_px = (target_end * pps).floor() as i64;

        let full = match self.sent {
            Some(s) => {
                s.width != width
                    || s.height != height
                    || s.px_per_second != pps
                    || s.rows_version != self.rows_version
                    || (end_px - s.end_px).unsigned_abs() as usize >= width
            }
            None => true,
        };
        let (shift, column_start, columns) = if full {
            (0i64, 0usize, width)
        } else {
            let shift = end_px - self.sent.map_or(end_px, |s| s.end_px);
            if shift > 0 {
                (shift, 0, shift as usize)
            } else if shift < 0 {
                (shift, (width as i64 + shift) as usize, (-shift) as usize)
            } else {
                (0, 0, 0)
            }
        };

        // Render the timeline columns, row-major RGBA.
        self.packet.clear();
        self.packet.resize(packet::packet_len(columns, height), 0);
        let (header, body) = self.packet.split_at_mut(packet::HEADER_LEN);
        let (pixels, live) = body.split_at_mut(columns * height * 4);
        for (x, col) in (column_start..column_start + columns).enumerate() {
            let t_hi = (end_px - col as i64) as f64 / pps;
            let t_lo = (end_px - col as i64 - 1) as f64 / pps;
            let first_tick = (t_lo * tps).floor().max(0.0) as u64;
            let last_tick = ((t_hi * tps).ceil() as u64).max(first_tick + 1) - 1;
            let last_tick = last_tick.min(first_tick + MAX_TICKS_PER_COLUMN as u64 - 1);
            let mut any = false;
            for tick in first_tick..=last_tick {
                if let Some(spectrum) = self.history.get(tick) {
                    if any {
                        for (i, v) in self.column.iter_mut().enumerate() {
                            *v = v.max(self.rows.value(i, spectrum));
                        }
                    } else {
                        for (i, v) in self.column.iter_mut().enumerate() {
                            *v = self.rows.value(i, spectrum);
                        }
                        any = true;
                    }
                }
            }
            for y in 0..height {
                let o = (y * columns + x) * 4;
                let v = if any {
                    self.column[height - 1 - y]
                } else {
                    0.0
                };
                if v > 0.0 {
                    pixels[o..o + 3].copy_from_slice(&self.lut[lut_index(v)]);
                }
                pixels[o + 3] = 255;
            }
        }
        for y in 0..height {
            let v = self.values[height - 1 - y];
            live[y * 4..y * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }

        self.seq = self.seq.wrapping_add(1);
        let mut flags = 0;
        if fresh {
            flags |= FLAG_NEW_AUDIO;
        }
        if following {
            flags |= FLAG_LIVE;
        }
        if full {
            flags |= FLAG_FULL;
        }
        Header {
            seq: self.seq,
            height: height as u32,
            flags,
            pitch_hz: pitch.hz as f32,
            target_hz: pitch.target_hz as f32,
            note: pitch.note.map_or(-1, |n| n as i32),
            sample_rate: self.config.sample_rate as f32,
            shift: shift as i32,
            column_start: column_start as u32,
            columns: columns as u32,
            width: width as u32,
            view_end: (end_px as f64 / pps) as f32,
            history_start: history_start as f32,
            history_end: history_end as f32,
            px_per_second: pps as f32,
        }
        .write(header);
        self.sent = Some(SentView {
            width,
            height,
            px_per_second: pps,
            end_px,
            rows_version: self.rows_version,
        });
        &self.packet
    }

    /// Row amplitudes (index 0 = lowest frequency) of the newest spectrum.
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize, freq: f32, sample_rate: f32, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    fn live_view(width: usize, pps: f64) -> ViewRequest {
        ViewRequest {
            width,
            px_per_second: pps,
            view_end: None,
        }
    }

    struct Parsed {
        seq: u32,
        flags: u32,
        pitch_hz: f32,
        note: i32,
        shift: i32,
        column_start: u32,
        columns: u32,
        width: u32,
        view_end: f32,
        history_end: f32,
        pixels: Vec<u8>,
        live: Vec<f32>,
    }

    fn parse(p: &[u8]) -> Parsed {
        let u = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        assert_eq!(u(0), packet::MAGIC);
        assert_eq!(u(4), packet::VERSION);
        let height = u(12) as usize;
        let columns = u(40) as usize;
        let live_off = packet::live_offset(columns, height);
        assert_eq!(p.len(), packet::packet_len(columns, height));
        Parsed {
            seq: u(8),
            flags: u(16),
            pitch_hz: f(20),
            note: i32::from_le_bytes(p[28..32].try_into().unwrap()),
            shift: i32::from_le_bytes(p[36..40].try_into().unwrap()),
            column_start: u(64),
            columns: u(40),
            width: u(44),
            view_end: f(48),
            history_end: f(56),
            pixels: p[packet::HEADER_LEN..live_off].to_vec(),
            live: (0..height).map(|y| f(live_off + y * 4)).collect(),
        }
    }

    #[test]
    fn first_frame_is_a_full_black_redraw() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        let p = parse(e.frame(live_view(100, 120.0)));
        assert_eq!(p.seq, 1);
        assert_ne!(p.flags & FLAG_FULL, 0);
        assert_ne!(p.flags & FLAG_LIVE, 0);
        assert_eq!(p.flags & FLAG_NEW_AUDIO, 0);
        assert_eq!(p.columns, 100);
        assert_eq!(p.width, 100);
        assert_eq!(p.note, -1);
        assert!(p.pixels.chunks_exact(4).all(|px| px == [0, 0, 0, 255]));
        assert!(p.live.iter().all(|&v| v == 0.0));
        // nothing changed: no columns
        let p = parse(e.frame(live_view(100, 120.0)));
        assert_eq!(p.columns, 0);
        assert_eq!(p.shift, 0);
    }

    #[test]
    fn live_view_advances_with_audio_time() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        e.frame(live_view(200, 100.0));
        // 94 hops (0.501 s) of tone: 100 px/s -> shift 50
        e.push_mono(&sine(94 * hop(8192), 440.0, 48000.0, 0.05));
        let p = parse(e.frame(live_view(200, 100.0)));
        assert_ne!(p.flags & FLAG_NEW_AUDIO, 0);
        assert_eq!(p.flags & FLAG_FULL, 0);
        assert_eq!(p.shift, 50);
        assert_eq!(p.column_start, 0);
        assert_eq!(p.columns, 50);
        assert!((p.history_end - 0.5).abs() < 0.01);
        assert!((p.view_end - 0.5).abs() < 0.01);
        assert!(
            (p.pitch_hz - 440.0).abs() < 1.0,
            "pitch {p:.1}",
            p = p.pitch_hz
        );
        // the 440 Hz row is lit in every new column; the first tick's window
        // is still mostly silence so check the newest column (x = 0)
        let row = e.rows.hz.iter().position(|&h| h >= 440.0).unwrap();
        let y = 546 - 1 - row;
        let px = &p.pixels[(y * 50) * 4..(y * 50) * 4 + 4];
        assert!(px[0] > 0 || px[1] > 0 || px[2] > 0, "row not lit: {px:?}");
        let quiet_y = 546 - 1 - e.rows.hz.iter().position(|&h| h >= 3000.0).unwrap();
        let px = &p.pixels[(quiet_y * 50) * 4..(quiet_y * 50) * 4 + 4];
        assert_eq!(&px[..3], &[0, 0, 0]);
    }

    #[test]
    fn scrolling_back_shifts_left_and_leaves_live_mode() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        e.frame(live_view(200, 100.0));
        e.push_mono(&vec![0.0; 96000]); // 2 s
        let p = parse(e.frame(live_view(200, 100.0)));
        assert_ne!(p.flags & FLAG_FULL, 0, "2 s = 200 px = a full redraw");
        // look 0.3 s back
        let p = parse(e.frame(ViewRequest {
            width: 200,
            px_per_second: 100.0,
            view_end: Some(1.7),
        }));
        assert_eq!(p.flags & FLAG_LIVE, 0);
        assert_eq!(p.shift, -30);
        assert_eq!(p.column_start, 170);
        assert_eq!(p.columns, 30);
        // asking for a time before the history clamps to its start
        let p = parse(e.frame(ViewRequest {
            width: 200,
            px_per_second: 100.0,
            view_end: Some(-5.0),
        }));
        assert!((p.view_end - 0.0).abs() < 1e-6);
        // scrolling to the newest edge re-enters live mode
        let p = parse(e.frame(ViewRequest {
            width: 200,
            px_per_second: 100.0,
            view_end: Some(99.0),
        }));
        assert_ne!(p.flags & FLAG_LIVE, 0);
        assert!((p.view_end - 2.0).abs() < 0.011);
    }

    #[test]
    fn zoom_and_speed_changes_force_a_full_redraw() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        e.frame(live_view(200, 100.0));
        let p = parse(e.frame(live_view(200, 50.0)));
        assert_ne!(p.flags & FLAG_FULL, 0);
        let p = parse(e.frame(live_view(200, 50.0)));
        assert_eq!(p.flags & FLAG_FULL, 0);
        let mut c = e.config().clone();
        c.range = Some(FreqRange::new(200.0, 800.0));
        e.configure(c).unwrap();
        let p = parse(e.frame(live_view(200, 50.0)));
        assert_ne!(p.flags & FLAG_FULL, 0);
        assert_eq!(e.info().range, FreqRange::new(200.0, 800.0));
    }

    #[test]
    fn harmonic_tone_reports_its_fundamental() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        let sr = 48000.0;
        let mut samples = vec![0.0f32; 8192];
        for (k, amp) in [(1.0, 0.03), (2.0, 0.03), (3.0, 0.02), (4.0, 0.015)] {
            for (s, v) in samples.iter_mut().zip(sine(8192, 110.0 * k, sr, amp)) {
                *s += v;
            }
        }
        e.push_mono(&samples);
        let p = parse(e.frame(live_view(100, 60.0)));
        assert!((p.pitch_hz - 110.0).abs() < 1.0, "pitch {}", p.pitch_hz);
        assert_eq!(e.info().notes[p.note as usize], "A2");
    }

    #[test]
    fn log_scale_stops_at_20k() {
        let e = Engine::new(EngineConfig {
            scale: Scale::Logarithmic,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(e.info().range, FreqRange::new(20.0, 20000.0));
    }

    #[test]
    fn reconfigure_keeps_history_unless_fft_changes() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        e.push_mono(&vec![0.0; 48000]);
        e.frame(live_view(100, 60.0));
        assert!((e.history_span().1 - 1.0).abs() < 0.01);
        let mut c = e.config().clone();
        c.labeling = Labeling::Linear;
        c.height = 1092;
        e.configure(c.clone()).unwrap();
        assert!((e.history_span().1 - 1.0).abs() < 0.01);
        assert_eq!(e.label_strip().len(), LABEL_WIDTH * 1092 * 4);
        c.fft_size = 16384;
        e.configure(c).unwrap();
        assert_eq!(e.history_span().1, 0.0);
        let info = e.info();
        assert_eq!(info.fft_size, 16384);
        assert!((info.ticks_per_second - 93.75).abs() < 1e-9);
        assert!(info.history_seconds > 60.0);
        assert!(e
            .configure(EngineConfig {
                fft_size: 1000,
                ..Default::default()
            })
            .is_err());
    }

    #[test]
    fn hidden_window_does_not_accumulate_unbounded_audio() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        for _ in 0..20 {
            e.push_mono(&vec![0.0; 48000]);
        }
        assert!(e.pending.len() <= 48000 * 8);
    }
}
