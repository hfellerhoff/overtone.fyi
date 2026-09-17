//! The analysis engine: owns the analyser, the recorded spectrum history and
//! all derived lookup tables, and renders the requested view of the timeline
//! into one binary frame packet (see [`crate::packet`]).
//!
//! Time is measured in audio samples. Analysis runs every [`hop`] samples,
//! independent of how often frames are requested, and each tick's spectrum is
//! kept in a [`History`] ring so the display can scroll back in time and be
//! re-rendered at a different frequency range.

use crate::analyser::Analyser;
use crate::audio::AudioRing;
use crate::color::{build_lut, lut_index, Coloring};
use crate::history::History;
use crate::labels::{frequency_markers, render_label_strip, Labeling, Marker};
use crate::mapping::{FreqRange, RowMap, Scale};
use crate::packet::{self, Header, FLAG_FULL, FLAG_LIVE, FLAG_NEW_AUDIO};
use crate::pitch::{
    detect_pitch, overtone_buckets, pitch_table, spectral_peaks, Bucket, Pitch, PitchResult,
};
use crate::spectrum::{default_bands, validate_bands, Band, Layout};
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
/// Default memory budget for retained raw audio, used to re-analyse when
/// the analysis settings change (4 bytes per sample: 64 MB ≈ 5.8 min at 48 kHz).
pub const DEFAULT_AUDIO_BYTES: usize = 64 << 20;
/// After an analysis change this much of the newest audio is re-analysed
/// before returning; the rest is filled in across later frames.
pub const SYNC_REANALYSIS_SECONDS: f64 = 4.0;
/// Seconds of old audio re-analysed per frame while a re-analysis is pending.
pub const REANALYSIS_SECONDS_PER_FRAME: f64 = 0.5;
/// Ticks folded into one timeline column are capped here when zoomed far out.
pub const MAX_TICKS_PER_COLUMN: usize = 64;

/// Ticks are never recorded faster than this; nothing above it is displayable.
pub const MAX_TICKS_PER_SECOND: f64 = 400.0;
/// Each band's FFT is recomputed once this fraction of its window has
/// arrived (15/16 overlap), regardless of how often ticks happen.
pub const ANALYSIS_OVERLAP_DIVISOR: usize = 16;

fn default_history_bytes() -> usize {
    DEFAULT_HISTORY_BYTES
}

fn default_audio_bytes() -> usize {
    DEFAULT_AUDIO_BYTES
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
    /// Memory budget for retained raw audio.
    #[serde(default = "default_audio_bytes")]
    pub audio_bytes: usize,
    /// Frequency bands and the window length used for each, as a divisor of
    /// `fft_size`. Defaults to halving the window per octave above 250 Hz.
    #[serde(default = "default_bands")]
    pub bands: Vec<Band>,
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
        validate_bands(&self.bands, self.fft_size)
    }

    pub fn layout(&self) -> Layout {
        Layout::build(self.sample_rate, self.fft_size, &self.bands)
    }

    /// Samples between analysis ticks: a quarter of the shortest window so
    /// treble events are not skipped, bounded by [`MAX_TICKS_PER_SECOND`] and
    /// never coarser than 1/32 of the base window.
    pub fn hop(&self) -> usize {
        let shortest = self
            .bands
            .iter()
            .map(|b| self.fft_size / b.divisor)
            .min()
            .unwrap_or(self.fft_size);
        let min_hop = (self.sample_rate / MAX_TICKS_PER_SECOND) as usize;
        (shortest / 4).max(min_hop).min(self.fft_size / 32).max(32)
    }

    /// Analysis ticks per second.
    pub fn ticks_per_second(&self) -> f64 {
        self.sample_rate / self.hop() as f64
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
            audio_bytes: DEFAULT_AUDIO_BYTES,
            bands: default_bands(),
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
    /// Seconds of raw audio retained for re-analysis.
    pub audio_seconds: f64,
    /// Bands in use: `[minHz, maxHz, fftSize]` per band, ascending.
    pub bands: Vec<(f64, f64, usize)>,
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

/// Progress of an incremental re-analysis of older audio, newest first.
struct Reanalysis {
    /// Ticks below this index still hold silence and need re-analysing.
    next_end: u64,
    analysers: Vec<Analyser>,
}

pub struct Engine {
    config: EngineConfig,
    /// One analyser per distinct window length, largest first.
    analysers: Vec<Analyser>,
    layout: Layout,
    composite: Vec<u8>,
    /// Scratch: element-wise max of the ticks folded into one column.
    fold: Vec<u8>,
    history: History,
    /// Raw audio, the source everything else is derived from.
    audio: AudioRing,
    /// Index into `audio` of the next sample to analyse.
    analysed_to: u64,
    /// Audio-sample index at which tick 0 of `history` starts.
    history_origin: u64,
    /// Scratch for feeding hops into the analysers.
    hop_buf: Vec<f32>,
    reanalysis: Option<Reanalysis>,
    /// Tick range rewritten since the last frame, to be repainted.
    dirty: Option<(u64, u64)>,
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
        let layout = config.layout();
        let analysers = layout.fft_sizes().into_iter().map(Analyser::new).collect();
        let rows = RowMap::build(config.effective_range(), config.height, &layout);
        let pitches = pitch_table();
        let mut label_strip = Vec::new();
        render_label_strip(&rows, config.labeling, LABEL_WIDTH, &mut label_strip);
        let lut = build_lut(config.coloring);
        let height = config.height;
        let history = History::new(layout.len, config.history_bytes);
        let audio = AudioRing::new(config.audio_bytes / 4);
        Ok(Self {
            config,
            analysers,
            composite: vec![0; layout.len],
            fold: vec![0; layout.len],
            layout,
            history,
            audio,
            analysed_to: 0,
            history_origin: 0,
            hop_buf: Vec::new(),
            reanalysis: None,
            dirty: None,
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

    /// Apply a new configuration, rebuilding only what changed. When the
    /// analysis itself changes (window sizes, bands, sample rate) the
    /// retained audio is re-analysed so the display carries over.
    pub fn configure(&mut self, config: EngineConfig) -> Result<(), String> {
        config.validate()?;
        let old = std::mem::replace(&mut self.config, config);
        let c = self.config.clone();
        let layout = c.layout();
        let layout_changed = layout != self.layout;
        if layout_changed {
            self.analysers = layout.fft_sizes().into_iter().map(Analyser::new).collect();
            self.composite = vec![0; layout.len];
            self.fold = vec![0; layout.len];
            self.layout = layout;
        }
        if c.audio_bytes != old.audio_bytes {
            self.audio = self.audio.resized(c.audio_bytes / 4);
        }
        let history_changed = layout_changed
            || c.sample_rate != old.sample_rate
            || c.history_bytes != old.history_bytes;
        if history_changed {
            self.history = History::new(self.layout.len, c.history_bytes);
            self.sent = None;
            self.reanalyse();
        }
        let rows_changed = layout_changed
            || c.height != old.height
            || c.effective_range() != old.effective_range();
        if rows_changed {
            self.rows = RowMap::build(c.effective_range(), c.height, &self.layout);
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
            audio_seconds: self.audio.capacity() as f64 / self.config.sample_rate,
            bands: self
                .layout
                .segments
                .iter()
                .map(|s| (s.min_hz, s.max_hz, s.fft_size))
                .collect(),
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
        self.audio.push(samples);
        // If frames stopped being requested, skip ahead rather than analysing
        // an unbounded backlog later.
        let max_pending = (self.config.sample_rate * MAX_PENDING_SECONDS) as u64;
        if self.audio.total() - self.analysed_to > max_pending {
            self.skip_to(self.audio.total() - max_pending);
        }
    }

    /// Move the analysis cursor forward to `sample`, recording silent ticks
    /// for the skipped span so history stays aligned with audio time.
    fn skip_to(&mut self, sample: u64) {
        let hop = self.config.hop() as u64;
        let target_tick = (sample - self.history_origin) / hop;
        let ticks_now = self.history.total();
        if target_tick > ticks_now {
            self.composite.iter_mut().for_each(|b| *b = 0);
            let n = (target_tick - ticks_now).min(self.history.capacity() as u64);
            for _ in 0..n {
                self.history.push(&self.composite);
            }
            self.analysers.iter_mut().for_each(Analyser::clear);
        }
        self.analysed_to = self.history_origin + target_tick * hop;
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
        self.analysers.iter_mut().for_each(Analyser::clear);
        self.history.clear();
        self.audio.clear();
        self.analysed_to = 0;
        self.history_origin = 0;
        self.reanalysis = None;
        self.dirty = None;
        self.sent = None;
    }

    /// Whether older audio is still being re-analysed in the background.
    pub fn reanalysis_pending(&self) -> bool {
        self.reanalysis.is_some()
    }

    /// Rebuild the spectrum history from the retained audio under the
    /// current layout. The newest [`SYNC_REANALYSIS_SECONDS`] are analysed
    /// now; older ticks start as silence and are filled in by
    /// [`Self::continue_reanalysis`] over later frames. Audio older than the
    /// ring is silence but keeps its place in time.
    fn reanalyse(&mut self) {
        self.analysers.iter_mut().for_each(Analyser::clear);
        let hop = self.config.hop() as u64;
        let first = self.audio.first();
        let newest = self.audio.total();
        // Only what the history budget can hold.
        let max_span = self.history.capacity() as u64 * hop;
        let origin = first.max(newest.saturating_sub(max_span));
        self.history_origin = origin;
        let total_ticks = (newest - origin) / hop;
        let sync_ticks = (SYNC_REANALYSIS_SECONDS * self.config.ticks_per_second()) as u64;
        let sync_start = total_ticks.saturating_sub(sync_ticks);
        self.history.push_silence(sync_start);
        self.reanalysis = if sync_start > 0 {
            Some(Reanalysis {
                next_end: sync_start,
                analysers: self
                    .layout
                    .fft_sizes()
                    .into_iter()
                    .map(Analyser::new)
                    .collect(),
            })
        } else {
            None
        };
        self.dirty = None;
        let start = origin + sync_start * hop;
        self.warm(start);
        self.analysed_to = start;
        self.process_pending();
    }

    /// Prime the live analysers with the window of audio preceding `start`.
    fn warm(&mut self, start: u64) {
        let warm = self
            .analysers
            .first()
            .map_or(0, |a| a.fft_size() as u64)
            .min(start - self.audio.first().min(start));
        if warm > 0 {
            let mut buf = vec![0.0f32; warm as usize];
            self.audio.read(start - warm, &mut buf);
            for a in &mut self.analysers {
                a.push_mono(&buf);
            }
        }
    }

    /// Re-analyse up to `max_ticks` older ticks (newest first) and mark them
    /// dirty. Returns how many were done.
    pub fn continue_reanalysis(&mut self, max_ticks: u64) -> u64 {
        let Some(mut job) = self.reanalysis.take() else {
            return 0;
        };
        let hop = self.config.hop() as u64;
        let origin = self.history_origin;
        let end = job.next_end;
        let start = end.saturating_sub(max_ticks.max(1));
        // warm the job's analysers with the window before the chunk
        let chunk_start = origin + start * hop;
        let warm = job
            .analysers
            .first()
            .map_or(0, |a| a.fft_size() as u64)
            .min(chunk_start - self.audio.first().min(chunk_start));
        job.analysers.iter_mut().for_each(Analyser::clear);
        if warm > 0 {
            let mut buf = vec![0.0f32; warm as usize];
            self.audio.read(chunk_start - warm, &mut buf);
            for a in &mut job.analysers {
                a.push_mono(&buf);
            }
        }
        self.hop_buf.resize(hop as usize, 0.0);
        for tick in start..end {
            self.audio.read(origin + tick * hop, &mut self.hop_buf);
            for a in &mut job.analysers {
                a.push_mono(&self.hop_buf);
                if a.samples_since_analysis() >= a.fft_size() / ANALYSIS_OVERLAP_DIVISOR {
                    a.byte_frequency_data();
                }
            }
            let analysers = &job.analysers;
            self.layout.compose(
                |n| {
                    analysers
                        .iter()
                        .find(|a| a.fft_size() == n)
                        .expect("an analyser exists for every band")
                        .bytes()
                },
                &mut self.composite,
            );
            self.history.set(tick, &self.composite);
        }
        self.dirty = Some(match self.dirty {
            Some((lo, hi)) => (lo.min(start), hi.max(end)),
            None => (start, end),
        });
        if start > 0 && start > self.history.first() {
            job.next_end = start;
            self.reanalysis = Some(job);
        }
        end - start
    }

    /// Length in seconds of the tick grid's origin offset within the audio
    /// ring, for tests.
    #[cfg(test)]
    fn history_origin_seconds(&self) -> f64 {
        self.history_origin as f64 / self.config.sample_rate
    }

    pub fn last_pitch(&self) -> PitchResult {
        self.last_pitch
    }

    /// Run analysis on all complete hops of pending audio. Returns how many
    /// ticks were recorded.
    pub fn process_pending(&mut self) -> usize {
        let hop = self.config.hop();
        let mut ticks = 0;
        if self.analysed_to < self.audio.first() {
            // fell behind the ring: the gap is silence
            self.skip_to(self.audio.first());
        }
        self.hop_buf.resize(hop, 0.0);
        while self.analysed_to + hop as u64 <= self.audio.total() {
            self.audio.read(self.analysed_to, &mut self.hop_buf);
            let chunk = &self.hop_buf;
            for a in &mut self.analysers {
                a.push_mono(chunk);
                if a.samples_since_analysis() >= a.fft_size() / ANALYSIS_OVERLAP_DIVISOR {
                    a.byte_frequency_data();
                }
            }
            let analysers = &self.analysers;
            self.layout.compose(
                |n| {
                    analysers
                        .iter()
                        .find(|a| a.fft_size() == n)
                        .expect("an analyser exists for every band")
                        .bytes()
                },
                &mut self.composite,
            );
            self.history.push(&self.composite);
            self.analysed_to += hop as u64;
            ticks += 1;
        }
        ticks
    }

    /// Seconds of audio time at which history tick 0 starts.
    fn origin_seconds(&self) -> f64 {
        self.history_origin as f64 / self.config.sample_rate
    }

    /// Oldest and newest recorded time in seconds (audio time, so the
    /// timeline keeps its position across re-analysis).
    pub fn history_span(&self) -> (f64, f64) {
        let tps = self.config.ticks_per_second();
        let origin = self.origin_seconds();
        (
            origin + self.history.first() as f64 / tps,
            origin + self.history.total() as f64 / tps,
        )
    }

    /// Analyse pending audio and render the requested view into a packet.
    pub fn frame(&mut self, view: ViewRequest) -> &[u8] {
        let fresh = self.process_pending() > 0;
        let height = self.config.height;
        let tps = self.config.ticks_per_second();
        if self.reanalysis.is_some() {
            self.continue_reanalysis((REANALYSIS_SECONDS_PER_FRAME * tps) as u64);
        }
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
                // The preset (not the zoomed) range is used so zooming the
                // display does not change what the pitch detector hears.
                let detect = FreqRange::preset(self.config.scale, self.config.sample_rate);
                self.peaks.clear();
                for seg in &self.layout.segments {
                    spectral_peaks(
                        &spectrum[seg.offset..seg.end()],
                        seg.bin_hz,
                        seg.first_bin as f64 * seg.bin_hz,
                        detect.min_hz.max(seg.min_hz),
                        detect.max_hz.min(seg.max_hz),
                        &mut self.peaks,
                    );
                }
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
        let origin = self.history_origin as f64 / self.config.sample_rate;
        let (shift, column_start, columns) = if full {
            (0i64, 0usize, width)
        } else {
            let shift = end_px - self.sent.map_or(end_px, |s| s.end_px);
            let (mut lo, mut hi) = if shift > 0 {
                (0usize, shift as usize)
            } else if shift < 0 {
                ((width as i64 + shift) as usize, width)
            } else {
                (width, width) // empty
            };
            // Columns covering re-analysed ticks must be repainted too; the
            // packet carries one block, so widen it to include them.
            if let Some((t0, t1)) = self.dirty.take() {
                let time_lo = origin + t0 as f64 / tps;
                let time_hi = origin + t1 as f64 / tps;
                let c_lo = (end_px as f64 - time_hi * pps).floor().max(0.0) as usize;
                let c_hi = ((end_px as f64 - time_lo * pps).ceil() as usize + 1).min(width);
                if c_lo < c_hi {
                    if lo == width {
                        lo = c_lo;
                        hi = c_hi;
                    } else {
                        lo = lo.min(c_lo);
                        hi = hi.max(c_hi);
                    }
                }
            }
            if lo < hi {
                (shift, lo, hi - lo)
            } else {
                (shift, 0, 0)
            }
        };
        if full {
            self.dirty = None;
        }

        // Render the timeline columns, row-major RGBA.
        self.packet.clear();
        self.packet.resize(packet::packet_len(columns, height), 0);
        let (header, body) = self.packet.split_at_mut(packet::HEADER_LEN);
        let (pixels, live) = body.split_at_mut(columns * height * 4);
        for (x, col) in (column_start..column_start + columns).enumerate() {
            let t_hi = (end_px - col as i64) as f64 / pps - origin;
            let t_lo = (end_px - col as i64 - 1) as f64 / pps - origin;
            let first_tick = (t_lo * tps).floor().max(0.0) as u64;
            let last_tick = ((t_hi * tps).ceil().max(0.0) as u64).max(first_tick + 1) - 1;
            let last_tick = last_tick.min(first_tick + MAX_TICKS_PER_COLUMN as u64 - 1);
            // Fold the column's ticks together on the (small) composite
            // spectrum first, then map rows once.
            let mut any = false;
            for tick in first_tick..=last_tick {
                if let Some(spectrum) = self.history.get(tick) {
                    if any {
                        for (f, &v) in self.fold.iter_mut().zip(spectrum) {
                            *f = (*f).max(v);
                        }
                    } else {
                        self.fold.copy_from_slice(spectrum);
                        any = true;
                    }
                }
            }
            if any {
                for (i, v) in self.column.iter_mut().enumerate() {
                    *v = self.rows.value(i, &self.fold);
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
        // just over half a second of tone: 100 px/s -> shift 50
        let hop = e.config().hop();
        let hops = 24000usize.div_ceil(hop);
        e.push_mono(&sine(hops * hop, 440.0, 48000.0, 0.05));
        let p = parse(e.frame(live_view(200, 100.0)));
        assert_ne!(p.flags & FLAG_NEW_AUDIO, 0);
        assert_eq!(p.flags & FLAG_FULL, 0);
        assert_eq!(p.shift, 50);
        assert_eq!(p.column_start, 0);
        assert_eq!(p.columns, 50);
        // spectrum + pitch on the multi-resolution default
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
    fn reconfigure_keeps_history_and_reanalyses_on_fft_change() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        // 1 s silence, 1 s of 440 Hz, 1 s silence
        let mut audio = vec![0.0f32; 48000];
        audio.extend(sine(48000, 440.0, 48000.0, 0.05));
        audio.extend(vec![0.0f32; 48000]);
        e.push_mono(&audio);
        e.frame(live_view(100, 60.0));
        assert!((e.history_span().1 - 3.0).abs() < 0.01);
        let mut c = e.config().clone();
        c.labeling = Labeling::Linear;
        c.height = 1092;
        e.configure(c.clone()).unwrap();
        assert!((e.history_span().1 - 3.0).abs() < 0.01);
        assert_eq!(e.label_strip().len(), LABEL_WIDTH * 1092 * 4);
        // changing the window size re-analyses the retained audio
        c.fft_size = 16384;
        e.configure(c).unwrap();
        assert!(
            (e.history_span().1 - 3.0).abs() < 0.01,
            "{:?}",
            e.history_span()
        );
        assert_eq!(e.history_origin_seconds(), 0.0);
        let row = e.rows.hz.iter().position(|&h| h >= 440.0).unwrap();
        let tps = e.config().ticks_per_second();
        let at = |e: &Engine, t: f64| e.rows.value(row, e.history.get((t * tps) as u64).unwrap());
        assert!(at(&e, 0.5) < 10.0, "silence at 0.5 s: {}", at(&e, 0.5));
        assert!(at(&e, 1.7) > 100.0, "tone at 1.7 s: {}", at(&e, 1.7));
        assert!(at(&e, 2.8) < 10.0, "silence at 2.8 s: {}", at(&e, 2.8));
        // and again for a band-layout change
        let mut c = e.config().clone();
        c.bands = crate::spectrum::BandPreset::Sharp.bands();
        e.configure(c).unwrap();
        assert!((e.history_span().1 - 3.0).abs() < 0.01);
        let tps = e.config().ticks_per_second();
        let row = e.rows.hz.iter().position(|&h| h >= 440.0).unwrap();
        let v = e
            .rows
            .value(row, e.history.get((1.7 * tps) as u64).unwrap());
        assert!(v > 100.0, "tone after band change: {v}");
        let info = e.info();
        assert_eq!(info.fft_size, 16384);
        // Sharp: shortest window is 16384 / 32 = 512; hop = 128 -> 375/s
        assert!((info.ticks_per_second - 375.0).abs() < 1e-9);
        assert!(info.history_seconds > 60.0);
        assert!(info.audio_seconds > 300.0);
        assert_eq!(info.bands.len(), 6);
        assert_eq!(info.bands[0], (0.0, 125.0, 16384));
        assert_eq!(info.bands[5], (2000.0, 24000.0, 512));
        assert!(e
            .configure(EngineConfig {
                fft_size: 1000,
                ..Default::default()
            })
            .is_err());
    }

    /// Feed a 20 ms burst at `hz` and count how many ticks it stays visible
    /// in the row nearest `hz`.
    fn burst_visible_ticks(e: &mut Engine, hz: f32) -> usize {
        e.clear();
        let sr = 48000.0;
        let burst = sine((0.020 * sr) as usize, hz, sr, 0.1);
        let mut signal = vec![0.0f32; 48000 * 3];
        signal[48000..48000 + burst.len()].copy_from_slice(&burst);
        e.push_mono(&signal);
        e.process_pending();
        let row = e.rows.hz.iter().position(|&h| h >= hz as f64).unwrap();
        let mut levels = Vec::new();
        for t in e.history.first()..e.history.total() {
            levels.push(e.rows.value(row, e.history.get(t).unwrap()));
        }
        let peak = levels.iter().cloned().fold(0.0, f32::max);
        levels.iter().filter(|&&l| l > peak * 0.5).count()
    }

    #[test]
    fn short_windows_keep_treble_events_sharp() {
        let mut multi = Engine::new(EngineConfig {
            fft_size: 32768,
            scale: Scale::Logarithmic,
            ..Default::default()
        })
        .unwrap();
        let mut single = Engine::new(EngineConfig {
            fft_size: 32768,
            scale: Scale::Logarithmic,
            bands: crate::spectrum::single_band(),
            ..Default::default()
        })
        .unwrap();
        let tps_multi = multi.config().ticks_per_second();
        let tps_single = single.config().ticks_per_second();
        let ms = |ticks: usize, tps: f64| ticks as f64 / tps * 1000.0;
        // 3 kHz sits in the 4096-sample band: ~85 ms window vs ~683 ms
        let m = ms(burst_visible_ticks(&mut multi, 3000.0), tps_multi);
        let s = ms(burst_visible_ticks(&mut single, 3000.0), tps_single);
        assert!(m < 120.0, "multi: {m} ms");
        assert!(s > 300.0, "single: {s} ms");
        // 100 Hz is still analysed with the full window in both
        let m = ms(burst_visible_ticks(&mut multi, 100.0), tps_multi);
        let s = ms(burst_visible_ticks(&mut single, 100.0), tps_single);
        assert!((m - s).abs() < 60.0, "multi {m} ms vs single {s} ms");
    }

    #[test]
    fn bands_scale_with_the_base_fft_size() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        assert_eq!(e.info().bands[3].2, 1024);
        let mut c = e.config().clone();
        c.fft_size = 32768;
        e.configure(c).unwrap();
        let bands = e.info().bands;
        assert_eq!(
            bands.iter().map(|b| b.2).collect::<Vec<_>>(),
            vec![32768, 16384, 8192, 4096]
        );
        let mut c = e.config().clone();
        c.bands = vec![
            Band {
                max_hz: Some(500.0),
                divisor: 1,
            },
            Band {
                max_hz: None,
                divisor: 16,
            },
        ];
        e.configure(c).unwrap();
        assert_eq!(
            e.info().bands,
            vec![(0.0, 500.0, 32768), (500.0, 24000.0, 2048)]
        );
        let mut c = e.config().clone();
        c.bands = vec![Band {
            max_hz: None,
            divisor: 5,
        }];
        assert!(e.configure(c).is_err());
    }

    #[test]
    fn hop_is_bounded_at_both_ends() {
        let tps = |fft: usize, bands: Vec<Band>| {
            EngineConfig {
                fft_size: fft,
                bands,
                ..Default::default()
            }
            .ticks_per_second()
        };
        use crate::spectrum::BandPreset::*;
        assert_eq!(tps(8192, Single.bands()), 187.5);
        assert_eq!(tps(8192, Balanced.bands()), 187.5);
        assert_eq!(tps(8192, Sharp.bands()), 400.0);
        assert_eq!(tps(32768, Single.bands()), 46.875);
        assert_eq!(tps(32768, Sharp.bands()), 187.5);
        assert_eq!(tps(4096, Sharp.bands()), 400.0);
    }

    #[test]
    fn big_windows_are_not_recomputed_every_tick() {
        // 8k sharp: hop 120 samples, the 8192 analyser refreshes every 512
        let mut e = Engine::new(EngineConfig {
            bands: crate::spectrum::BandPreset::Sharp.bands(),
            ..Default::default()
        })
        .unwrap();
        e.push_mono(&sine(48000, 440.0, 48000.0, 0.05));
        let t = std::time::Instant::now();
        let ticks = e.process_pending();
        let per_tick = t.elapsed().as_secs_f64() / ticks as f64;
        assert_eq!(ticks, 400);
        // generous bound (debug build): well under a millisecond per tick
        assert!(per_tick < 2e-3, "{per_tick} s per tick");
        // and the big band still produced data
        let row = e.rows.hz.iter().position(|&h| h >= 110.0).unwrap();
        let _ = row;
        assert!(e.history.latest().unwrap().iter().any(|&b| b > 0));
    }

    #[test]
    fn long_recordings_reanalyse_incrementally_and_repaint() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        // 30 s: tone for the first 10 s, silence, tone for the last 2 s
        let mut audio = sine(48000 * 10, 440.0, 48000.0, 0.05);
        audio.extend(vec![0.0f32; 48000 * 18]);
        audio.extend(sine(48000 * 2, 440.0, 48000.0, 0.05));
        for chunk in audio.chunks(48000) {
            e.push_mono(chunk);
            e.process_pending();
        }
        let view = live_view(600, 20.0); // 30 s across 600 px
        e.frame(view);
        let mut c = e.config().clone();
        c.bands = crate::spectrum::BandPreset::Sharp.bands();
        e.configure(c).unwrap();
        assert!(e.reanalysis_pending());
        let tps = e.config().ticks_per_second();
        let row = e.rows.hz.iter().position(|&h| h >= 440.0).unwrap();
        let at = |e: &Engine, t: f64| e.rows.value(row, e.history.get((t * tps) as u64).unwrap());
        // newest seconds are ready at once, the old tone is still silence
        assert!(at(&e, 29.0) > 100.0, "newest: {}", at(&e, 29.0));
        assert!(at(&e, 5.0) == 0.0, "old: {}", at(&e, 5.0));
        // first frame after a history rebuild is a full redraw
        let p = parse(e.frame(view));
        assert_ne!(p.flags & FLAG_FULL, 0);
        let lit = |p: &Parsed, x: usize| {
            let o = ((546 - 1 - row) * p.columns as usize + x) * 4;
            p.pixels[o] > 0 || p.pixels[o + 1] > 0 || p.pixels[o + 2] > 0
        };
        // frames keep re-analysing older audio and repaint those columns;
        // 26 s of backlog at 0.5 s per frame takes ~52 frames
        let mut repainted_old_tone = false;
        for _ in 0..80 {
            if !e.reanalysis_pending() {
                break;
            }
            let p = parse(e.frame(view));
            // column for t = 5 s is 25 s back = 500 px from the newest edge
            if p.columns > 0
                && p.column_start as usize <= 500
                && (p.column_start + p.columns) as usize > 500
            {
                let x = 500 - p.column_start as usize;
                repainted_old_tone |= lit(&p, x);
            }
        }
        assert!(!e.reanalysis_pending());
        assert!(at(&e, 5.0) > 100.0, "old after job: {}", at(&e, 5.0));
        assert!(at(&e, 15.0) == 0.0, "silence after job: {}", at(&e, 15.0));
        assert!(
            repainted_old_tone,
            "the old tone's column was never repainted"
        );
        // idle again afterwards
        let p = parse(e.frame(view));
        assert_eq!(p.columns, 0);
    }

    #[test]
    fn reanalysis_is_bounded_by_the_audio_ring() {
        // tiny audio ring: 0.5 s; history is 2 s old when the layout changes
        let mut e = Engine::new(EngineConfig {
            audio_bytes: 48000 * 4 / 2,
            ..Default::default()
        })
        .unwrap();
        e.push_mono(&sine(48000 * 2, 440.0, 48000.0, 0.05));
        e.frame(live_view(100, 60.0));
        let mut c = e.config().clone();
        c.bands = crate::spectrum::BandPreset::Single.bands();
        e.configure(c).unwrap();
        let (start, end) = e.history_span();
        assert!((end - 2.0).abs() < 0.02, "end {end}");
        assert!((start - 1.5).abs() < 0.02, "start {start}");
        // the re-analysed part still shows the tone and the timeline is
        // positioned in audio time, not restarted at zero
        let row = e.rows.hz.iter().position(|&h| h >= 440.0).unwrap();
        assert!(e.rows.value(row, e.history.latest().unwrap()) > 100.0);
        let p = parse(e.frame(live_view(100, 60.0)));
        assert!((p.history_end - 2.0).abs() < 0.02);
        assert!((p.view_end - 2.0).abs() < 0.02);
    }

    #[test]
    fn hidden_window_does_not_accumulate_unbounded_audio() {
        let mut e = Engine::new(EngineConfig::default()).unwrap();
        for _ in 0..20 {
            e.push_mono(&vec![0.0; 48000]);
        }
        assert!(e.audio.total() - e.analysed_to <= 48000 * 8);
        // the skipped span is still accounted for in time
        e.process_pending();
        assert!((e.history_span().1 - 20.0).abs() < 0.05);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    #[test]
    #[ignore]
    fn tick_cost() {
        for &fft in &[8192usize, 16384, 32768] {
            for bands in [crate::spectrum::single_band(), default_bands()] {
                let mut e = Engine::new(EngineConfig {
                    fft_size: fft,
                    bands: bands.clone(),
                    ..Default::default()
                })
                .unwrap();
                let samples: Vec<f32> = (0..48000 * 5)
                    .map(|i| 0.1 * (i as f32 * 0.05).sin())
                    .collect();
                let t = std::time::Instant::now();
                e.push_mono(&samples);
                let ticks = e.process_pending();
                let el = t.elapsed();
                eprintln!(
                    "fft {fft:>5} bands {} hop {:>4}: {ticks} ticks in {:?} = {:.1} µs/tick, {:.1}% of realtime, history {} B/tick, {:.0} s budget",
                    bands.len(),
                    e.config().hop(),
                    el,
                    el.as_secs_f64() * 1e6 / ticks as f64,
                    el.as_secs_f64() / 5.0 * 100.0,
                    e.layout.len,
                    e.info().history_seconds
                );
            }
        }
    }
}

#[cfg(test)]
mod reanalysis_bench {
    use super::*;

    #[test]
    #[ignore]
    fn reanalysis_cost() {
        for &fft in &[8192usize, 32768] {
            let mut e = Engine::new(EngineConfig {
                fft_size: fft,
                ..Default::default()
            })
            .unwrap();
            // fill the whole default audio ring (~5.8 min)
            let chunk: Vec<f32> = (0..48000).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
            let seconds = e.info().audio_seconds as usize;
            for _ in 0..seconds {
                e.push_mono(&chunk);
                e.process_pending();
            }
            for preset in [
                crate::spectrum::BandPreset::Single,
                crate::spectrum::BandPreset::Sharp,
                crate::spectrum::BandPreset::Balanced,
            ] {
                let mut c = e.config().clone();
                c.bands = preset.bands();
                let t = std::time::Instant::now();
                e.configure(c).unwrap();
                eprintln!(
                    "fft {fft:>5} -> {preset:?}: re-analysed {seconds} s of audio in {:?}",
                    t.elapsed()
                );
            }
        }
    }
}
