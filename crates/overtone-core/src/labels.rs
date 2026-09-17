//! Renders the frequency label strip (piano keys or linear gridlines) into an
//! RGBA buffer. Port of `useUpdateFrequencyLabelCanvas`; the strip only
//! depends on the configuration so it is rendered once per configuration
//! instead of once per frame.

use crate::pitch::Pitch;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Labeling {
    Linear,
    Piano,
}

const GRID_MINOR: [u8; 3] = [0x44, 0x44, 0x44];
const GRID_MAJOR: [u8; 3] = [0x99, 0x99, 0x99];
const OCTAVE_LINE: [u8; 3] = [0x8A, 0x8A, 0x8A];
const KEY_LINE: [u8; 3] = [0xDD, 0xDD, 0xDD];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const BLACK_KEY: [u8; 3] = [0x11, 0x11, 0x11];

struct Strip<'a> {
    width: usize,
    height: usize,
    out: &'a mut [u8],
}

impl Strip<'_> {
    /// `fillRect(x, y, w, 1)` with canvas clipping.
    fn fill_row(&mut self, x: usize, y: usize, w: usize, rgb: [u8; 3]) {
        if y >= self.height {
            return;
        }
        let x_end = (x + w).min(self.width);
        for px in x..x_end {
            let o = (y * self.width + px) * 4;
            self.out[o..o + 3].copy_from_slice(&rgb);
            self.out[o + 3] = 255;
        }
    }
}

pub fn render_label_strip(
    start_hz: &[f64],
    pitches: &[Pitch],
    labeling: Labeling,
    width: usize,
    out: &mut Vec<u8>,
) {
    let height = start_hz.len();
    out.clear();
    out.resize(width * height * 4, 0);
    // alpha:false canvas after clearRect is opaque black
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let mut strip = Strip { width, height, out };

    let mut fill_style = WHITE;
    let mut hundreds = 0i64;
    let mut thousands = 0i64;
    let mut pitch_index = 0usize;
    let mut key_is_black = false;

    for (i, &hz) in start_hz.iter().enumerate() {
        // canvas y coordinate; row 0 maps to y = height and is clipped
        let y = height - i;

        match labeling {
            Labeling::Linear => {
                let point_hundreds = (hz / 100.0).floor() as i64;
                let point_thousands = (hz / 1000.0).floor() as i64;
                if point_hundreds > hundreds {
                    hundreds = point_hundreds;
                    fill_style = GRID_MINOR;
                    strip.fill_row(0, y, width, fill_style);
                } else if point_thousands > thousands {
                    thousands = point_thousands;
                    fill_style = GRID_MAJOR;
                    strip.fill_row(0, y, width, fill_style);
                }
            }
            Labeling::Piano => {
                if pitch_index >= pitches.len() {
                    continue;
                }
                let mut pitch = Some(&pitches[pitch_index]);
                if pitch.is_some_and(|p| p.hz < hz) {
                    while pitch.is_some_and(|p| p.hz < hz) {
                        pitch_index += 1;
                        pitch = pitches.get(pitch_index);
                    }
                    if let Some(p) = pitch {
                        let label = &p.label;
                        if label.contains('C') && !label.contains('#') {
                            strip.fill_row(0, y, width, OCTAVE_LINE);
                            fill_style = WHITE;
                            key_is_black = false;
                        } else if label.contains('#') {
                            strip.fill_row(0, y, width, KEY_LINE);
                            fill_style = BLACK_KEY;
                            strip.fill_row(0, y, width / 2, fill_style);
                            key_is_black = true;
                        } else {
                            strip.fill_row(0, y, width, KEY_LINE);
                            fill_style = WHITE;
                            key_is_black = false;
                        }
                    }
                } else if key_is_black {
                    strip.fill_row(0, y, width / 2, fill_style);
                    fill_style = WHITE;
                    strip.fill_row(width / 2, y, width / 2, fill_style);
                    fill_style = BLACK_KEY;
                } else {
                    strip.fill_row(0, y, width, fill_style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{pixel_start, Scale};
    use crate::pitch::pitch_table;

    fn piano_rows(h: usize) -> Vec<f64> {
        (0..h)
            .map(|i| pixel_start(Scale::Piano, 24000.0, h, i))
            .collect()
    }

    fn px(out: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
        let o = (y * width + x) * 4;
        [out[o], out[o + 1], out[o + 2]]
    }

    #[test]
    fn piano_strip_has_black_and_white_keys_and_octave_lines() {
        let rows = piano_rows(1092);
        let mut out = Vec::new();
        render_label_strip(&rows, &pitch_table(), Labeling::Piano, 64, &mut out);
        assert_eq!(out.len(), 64 * 1092 * 4);
        // top row is never drawn
        assert_eq!(px(&out, 64, 10, 0), [0, 0, 0]);
        let mut black_half = 0;
        let mut octave_lines = 0;
        for y in 1..1092 {
            let left = px(&out, 64, 10, y);
            let right = px(&out, 64, 50, y);
            if left == BLACK_KEY && right == WHITE {
                black_half += 1;
            }
            if left == OCTAVE_LINE {
                octave_lines += 1;
            }
        }
        assert!(black_half > 200, "black key rows: {black_half}");
        assert_eq!(octave_lines, 8, "C1..C8 within 36Hz..9.4kHz");
    }

    #[test]
    fn linear_strip_draws_gridlines() {
        let rows = piano_rows(546);
        let mut out = Vec::new();
        render_label_strip(&rows, &pitch_table(), Labeling::Linear, 64, &mut out);
        let lines = (1..546)
            .filter(|&y| px(&out, 64, 3, y) == GRID_MINOR)
            .count();
        assert!(lines > 50);
    }
}
