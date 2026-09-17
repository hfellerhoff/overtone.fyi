//! Amplitude → colour for the scrolling spectrogram. Port of
//! `getAudioAmplitudeValueColor`, including the CSS `hsl()` → sRGB step that
//! the browser used to perform when the string was assigned to `fillStyle`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Coloring {
    Sigmoid,
    Detailed,
}

/// `Math.round` semantics (half rounds toward +∞), unlike Rust's `round`.
#[inline]
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn interpolate(
    value: f64,
    input_min: f64,
    input_max: f64,
    output_min: f64,
    output_max: f64,
) -> f64 {
    let adjusted_value = value - input_min;
    let adjusted_max = input_max - input_min;
    let ratio = adjusted_value / adjusted_max;
    ratio * (output_max - output_min) + output_min
}

fn sigmoid(x: f64) -> f64 {
    let a = 9.0;
    let b = -0.05;
    let numerator = 255.0;
    let power = a + b * x;
    let denominator = 1.0 + std::f64::consts::E.powf(power);
    numerator / denominator
}

/// CSS `hsl(h, 100%, l%)` → 8-bit sRGB, following the CSS Color 4 algorithm.
/// `hue` is in degrees (any value, wrapped), `lightness` is a percentage
/// (clamped to 0..=100 like the browser does).
pub fn hsl_to_rgb(hue: f64, saturation_pct: f64, lightness_pct: f64) -> [u8; 3] {
    let h = ((hue % 360.0) + 360.0) % 360.0;
    let s = (saturation_pct / 100.0).clamp(0.0, 1.0);
    let l = (lightness_pct / 100.0).clamp(0.0, 1.0);
    let a = s * l.min(1.0 - l);
    let f = |n: f64| {
        let k = (n + h / 30.0) % 12.0;
        l - a * (-1.0f64).max((k - 3.0).min(9.0 - k).min(1.0))
    };
    let to_byte = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    [to_byte(f(0.0)), to_byte(f(8.0)), to_byte(f(4.0))]
}

pub fn amplitude_color(value: f64, coloring: Coloring) -> [u8; 3] {
    let color_max = 255.0;
    let constrained_value = interpolate(value, 0.0, 255.0, 0.0, 1.0);
    let color_value = interpolate(constrained_value, 0.0, 1.0, 0.0, 255.0);

    match coloring {
        Coloring::Sigmoid => {
            let color_adjustment = 24.0;
            let adjusted_color_value = sigmoid(color_value + color_adjustment);
            let color = color_max - adjusted_color_value;
            let l = js_round(10.0 * adjusted_color_value.ln());
            hsl_to_rgb(color, 100.0, l)
        }
        Coloring::Detailed => {
            let color_adjustment = 16.0;
            if color_value == 0.0 {
                return [0, 0, 0];
            }
            let color = 255.0 - color_value - color_adjustment;
            let l = (color_value + color_adjustment - 128.0).abs().min(50.0);
            hsl_to_rgb(color, 100.0, l)
        }
    }
}

/// Row values are always multiples of 0.5 (the mean of two bytes), so a
/// 511-entry table covers every possible colour for a configuration.
pub fn build_lut(coloring: Coloring) -> Vec<[u8; 3]> {
    (0..=510)
        .map(|i| amplitude_color(i as f64 / 2.0, coloring))
        .collect()
}

#[inline]
pub fn lut_index(value: f32) -> usize {
    ((value * 2.0).round().max(0.0) as usize).min(510)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_matches_css_reference_values() {
        assert_eq!(hsl_to_rgb(0.0, 100.0, 50.0), [255, 0, 0]);
        assert_eq!(hsl_to_rgb(120.0, 100.0, 50.0), [0, 255, 0]);
        assert_eq!(hsl_to_rgb(240.0, 100.0, 50.0), [0, 0, 255]);
        assert_eq!(
            hsl_to_rgb(-10.0, 100.0, 50.0),
            hsl_to_rgb(350.0, 100.0, 50.0)
        );
        assert_eq!(hsl_to_rgb(200.0, 100.0, -5.0), [0, 0, 0]);
        assert_eq!(hsl_to_rgb(60.0, 100.0, 25.0), [128, 128, 0]);
    }

    #[test]
    fn detailed_zero_is_black_and_mid_is_black() {
        assert_eq!(amplitude_color(0.0, Coloring::Detailed), [0, 0, 0]);
        // lightness = |112 + 16 - 128| = 0
        assert_eq!(amplitude_color(112.0, Coloring::Detailed), [0, 0, 0]);
        assert_eq!(
            amplitude_color(255.0, Coloring::Detailed),
            hsl_to_rgb(-16.0, 100.0, 50.0)
        );
    }

    #[test]
    fn sigmoid_low_values_are_black_and_high_values_are_bright() {
        assert_eq!(amplitude_color(1.0, Coloring::Sigmoid), [0, 0, 0]);
        let [r, g, b] = amplitude_color(255.0, Coloring::Sigmoid);
        assert!(r > 100 || g > 100 || b > 100);
    }

    #[test]
    fn js_round_rounds_half_up() {
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-22.6), -23.0);
    }
}
