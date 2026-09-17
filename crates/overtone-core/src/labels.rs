//! Renders the frequency label strip (piano keys or Hz gridlines) into an
//! RGBA buffer, and picks the numeric frequency markers shown next to the
//! timeline. Both depend only on the row map, so they are rebuilt when the
//! configuration or the visible range changes, not per frame.

use crate::mapping::{note_number, RowMap};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Labeling {
    /// Hz gridlines: a major line per decade, minor lines at 2..9 × 10ⁿ.
    Linear,
    /// Piano keys, one per semitone, with a stronger line at every C.
    Piano,
}

const GRID_MINOR: [u8; 3] = [0x44, 0x44, 0x44];
const GRID_MAJOR: [u8; 3] = [0x99, 0x99, 0x99];
const OCTAVE_LINE: [u8; 3] = [0x8A, 0x8A, 0x8A];
const KEY_LINE: [u8; 3] = [0xDD, 0xDD, 0xDD];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const BLACK_KEY: [u8; 3] = [0x11, 0x11, 0x11];

/// Semitone offsets within an octave (from C) that are black keys.
const BLACK_KEYS: [bool; 12] = [
    false, true, false, true, false, false, true, false, true, false, true, false,
];

struct Strip<'a> {
    width: usize,
    height: usize,
    out: &'a mut [u8],
}

impl Strip<'_> {
    /// Fill `w` pixels of analysis row `i` (row 0 = bottom) starting at `x`.
    fn fill_row(&mut self, i: usize, x: usize, w: usize, rgb: [u8; 3]) {
        if i >= self.height {
            return;
        }
        let y = self.height - 1 - i;
        let x_end = (x + w).min(self.width);
        for px in x..x_end {
            let o = (y * self.width + px) * 4;
            self.out[o..o + 3].copy_from_slice(&rgb);
        }
    }
}

pub fn render_label_strip(rows: &RowMap, labeling: Labeling, width: usize, out: &mut Vec<u8>) {
    let height = rows.height();
    out.clear();
    out.resize(width * height * 4, 0);
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let mut strip = Strip { width, height, out };
    match labeling {
        Labeling::Linear => render_hz_grid(rows, &mut strip),
        Labeling::Piano => render_piano(rows, &mut strip),
    }
}

/// Index of the first row whose lower edge is at or above `hz`, if any row
/// contains `hz`. Edges are ascending, so this is a binary search.
fn row_at(rows: &RowMap, hz: f64) -> Option<usize> {
    if hz < rows.edges[0] || hz >= rows.edges[rows.height()] {
        return None;
    }
    let i = rows.edges.partition_point(|&e| e <= hz);
    Some(i - 1)
}

fn render_hz_grid(rows: &RowMap, strip: &mut Strip) {
    let width = strip.width;
    let min = rows.range.min_hz;
    let max = rows.range.max_hz;
    let rows_per_decade = strip.height as f64 / (max / min).log10();
    // Minor lines get too dense to read when a decade spans few rows.
    let draw_minor = rows_per_decade >= 40.0;
    let mut decade = 10f64.powf(min.log10().floor());
    while decade <= max {
        for mantissa in 1..10 {
            if mantissa > 1 && !draw_minor {
                break;
            }
            let hz = decade * mantissa as f64;
            if let Some(i) = row_at(rows, hz) {
                let colour = if mantissa == 1 {
                    GRID_MAJOR
                } else {
                    GRID_MINOR
                };
                strip.fill_row(i, 0, width, colour);
            }
        }
        decade *= 10.0;
    }
}

fn render_piano(rows: &RowMap, strip: &mut Strip) {
    let width = strip.width;
    let height = strip.height;
    let rows_per_semitone = height as f64 / (12.0 * (rows.range.max_hz / rows.range.min_hz).log2());

    if rows_per_semitone < 2.5 {
        // Too zoomed out for keys: white strip with a line at every C.
        for i in 0..height {
            strip.fill_row(i, 0, width, WHITE);
        }
        let first = note_number(rows.range.min_hz).ceil() as i64;
        let last = note_number(rows.range.max_hz).floor() as i64;
        for n in first..=last {
            if (n - 4).rem_euclid(12) == 0 {
                let hz = crate::mapping::pitch_by_number(n as f64);
                if let Some(i) = row_at(rows, hz) {
                    strip.fill_row(i, 0, width, OCTAVE_LINE);
                }
            }
        }
        return;
    }

    let mut previous_key: Option<i64> = None;
    for i in 0..height {
        // A key spans ±half a semitone around its pitch.
        let key = note_number(rows.hz[i]).round() as i64;
        // key 4 is C1, so (key - 4) mod 12 is the offset from C.
        let offset = (key - 4).rem_euclid(12) as usize;
        let is_black = BLACK_KEYS[offset];
        if is_black {
            strip.fill_row(i, 0, width / 2, BLACK_KEY);
            strip.fill_row(i, width / 2, width - width / 2, WHITE);
        } else {
            strip.fill_row(i, 0, width, WHITE);
        }
        if previous_key.is_some_and(|p| p != key) {
            // boundary between keys, on this key's first row
            let line = if offset == 0 { OCTAVE_LINE } else { KEY_LINE };
            let w = if is_black { width - width / 2 } else { width };
            let x = if is_black { width / 2 } else { 0 };
            strip.fill_row(i, x, w, line);
            if is_black {
                strip.fill_row(i, 0, width / 2, KEY_LINE);
            }
        }
        previous_key = Some(key);
    }
}

/// A numeric frequency marker next to the timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub hz: f64,
    pub label: String,
    /// Position from the bottom of the canvas, 0..=1.
    pub fraction: f64,
}

fn format_hz(hz: f64) -> String {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        if (k - k.round()).abs() < 1e-9 {
            format!("{}k", k.round() as i64)
        } else {
            format!("{k:.1}k")
        }
    } else if (hz - hz.round()).abs() < 1e-9 {
        format!("{}", hz.round() as i64)
    } else {
        format!("{hz:.1}")
    }
}

/// Choose readable "round" frequencies for the visible range. Candidates are
/// 1, 2, 3, 5, 7 (then 1.5, 4, 6, 8, 9) × 10ⁿ, added in priority order while
/// they stay at least `min_gap` (a fraction of the height) apart.
pub fn frequency_markers(rows: &RowMap, min_gap: f64) -> Vec<Marker> {
    let range = rows.range;
    let priorities: [f64; 10] = [1.0, 5.0, 2.0, 3.0, 7.0, 1.5, 4.0, 6.0, 8.0, 9.0];
    let mut chosen: Vec<Marker> = Vec::new();
    let start = range.min_hz.log10().floor() as i32;
    let end = range.max_hz.log10().ceil() as i32;
    for &mantissa in &priorities {
        for exp in start..=end {
            let hz = mantissa * 10f64.powi(exp);
            if hz < range.min_hz || hz > range.max_hz {
                continue;
            }
            let fraction = range.fraction_of(hz);
            if !(0.005..=0.995).contains(&fraction) {
                continue;
            }
            if chosen
                .iter()
                .all(|m| (m.fraction - fraction).abs() >= min_gap)
            {
                chosen.push(Marker {
                    hz,
                    label: format_hz(hz),
                    fraction,
                });
            }
        }
    }
    chosen.sort_by(|a, b| b.hz.partial_cmp(&a.hz).unwrap());
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{pitch_by_number, FreqRange, Scale};

    fn px(out: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
        let o = (y * width + x) * 4;
        [out[o], out[o + 1], out[o + 2]]
    }

    #[test]
    fn piano_keys_are_centred_on_their_pitches() {
        let rows = RowMap::build(
            FreqRange::preset(Scale::Piano, 48000.0),
            1092,
            48000.0,
            8192,
        );
        let mut out = Vec::new();
        render_label_strip(&rows, Labeling::Piano, 64, &mut out);
        assert_eq!(out.len(), 64 * 1092 * 4);
        let key_colour = |hz: f64| {
            let i = row_at(&rows, hz).unwrap();
            px(&out, 64, 10, 1092 - 1 - i)
        };
        // A4 (white) and A#4 (black) at their exact pitches, not at a boundary
        assert_eq!(key_colour(440.0), WHITE);
        assert_eq!(key_colour(466.16), BLACK_KEY);
        assert_eq!(key_colour(261.63), WHITE); // C4
                                               // the C4 key starts half a semitone below C4
        let c4_lower_edge = pitch_by_number(40.0 - 0.5);
        let i = row_at(&rows, c4_lower_edge).unwrap();
        let line = (i.saturating_sub(1)..=i + 1)
            .map(|r| px(&out, 64, 10, 1092 - 1 - r))
            .find(|c| *c == OCTAVE_LINE);
        assert_eq!(line, Some(OCTAVE_LINE));
        // every row is drawn, including the top and bottom ones
        assert_ne!(px(&out, 64, 10, 0), [0, 0, 0]);
        assert_ne!(px(&out, 64, 10, 1091), [0, 0, 0]);
        // 8 octave lines (C2..C9 are within 38.9 Hz..9.4 kHz)
        let octave_lines = (0..1092)
            .filter(|&y| px(&out, 64, 50, y) == OCTAVE_LINE)
            .count();
        assert_eq!(octave_lines, 8);
    }

    #[test]
    fn zoomed_out_piano_shows_only_octave_lines() {
        let rows = RowMap::build(FreqRange::new(20.0, 20000.0), 100, 48000.0, 8192);
        let mut out = Vec::new();
        render_label_strip(&rows, Labeling::Piano, 64, &mut out);
        let lines = (0..100)
            .filter(|&y| px(&out, 64, 5, y) == OCTAVE_LINE)
            .count();
        assert_eq!(lines, 10); // C1 (32.7) .. C10 (16744)
        assert!((0..100).all(|y| px(&out, 64, 5, y) != BLACK_KEY));
    }

    #[test]
    fn hz_grid_marks_decades_and_minor_lines() {
        let rows = RowMap::build(FreqRange::new(20.0, 20000.0), 1092, 48000.0, 8192);
        let mut out = Vec::new();
        render_label_strip(&rows, Labeling::Linear, 64, &mut out);
        let major = (0..1092)
            .filter(|&y| px(&out, 64, 5, y) == GRID_MAJOR)
            .count();
        let minor = (0..1092)
            .filter(|&y| px(&out, 64, 5, y) == GRID_MINOR)
            .count();
        assert_eq!(major, 3); // 100, 1k, 10k
        assert_eq!(minor, 24); // 20..90, 200..900, 2k..9k (20k is the top edge)
        let i = row_at(&rows, 1000.0).unwrap();
        assert_eq!(px(&out, 64, 5, 1092 - 1 - i), GRID_MAJOR);
    }

    #[test]
    fn markers_are_round_and_spaced() {
        let rows = RowMap::build(FreqRange::new(20.0, 20000.0), 1092, 48000.0, 8192);
        let markers = frequency_markers(&rows, 0.03);
        let labels: Vec<&str> = markers.iter().map(|m| m.label.as_str()).collect();
        assert!(labels.contains(&"100"));
        assert!(labels.contains(&"1k"));
        assert!(labels.contains(&"10k"));
        assert!(labels.contains(&"500"));
        assert!(markers.windows(2).all(|w| w[0].hz > w[1].hz));
        for w in markers.windows(2) {
            assert!((w[0].fraction - w[1].fraction).abs() >= 0.03);
        }
        assert_eq!(format_hz(1500.0), "1.5k");
        assert_eq!(format_hz(2000.0), "2k");
        assert_eq!(format_hz(38.9), "38.9");
    }
}
