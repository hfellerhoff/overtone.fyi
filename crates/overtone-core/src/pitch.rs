//! Overtone bucketing and pitch detection. Ports of `getOvertoneBuckets`,
//! `getPitch` and `findPitchLabel`, plus the generated `pitches` table.

use crate::mapping::pitch_by_number;

/// The percent similarity where two distinct Hz values are lumped into one bucket.
pub const BUCKET_SENSITIVITY: f64 = 0.9;
pub const BUCKET_MIN_AMPLITUDE: f64 = 100.0;
pub const PITCH_SENSITIVITY: f64 = 0.85;

const PITCH_LETTERS: [&str; 12] = [
    "C", "C#/Db", "D", "D#/Eb", "E", "F", "F#/Gb", "G", "G#/Ab", "A", "A#/Bb", "B",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Pitch {
    pub hz: f64,
    pub label: String,
}

/// The `pitches` table: every note from G#0 up through 20 octaves, ascending.
pub fn pitch_table() -> Vec<Pitch> {
    let octaves = 20;
    let pitches_in_octave = PITCH_LETTERS.len();
    let number_offset = PITCH_LETTERS.iter().position(|l| *l == "A").unwrap() as i64;
    let mut out = Vec::new();
    for o in 0..octaves {
        for (i, letters) in PITCH_LETTERS.iter().enumerate() {
            let note_number = (o * pitches_in_octave + i) as i64 - number_offset + 1;
            if note_number < 0 {
                continue;
            }
            let hz = pitch_by_number(note_number as f64);
            let label = letters
                .split('/')
                .map(|part| format!("{part}{o}"))
                .collect::<Vec<_>>()
                .join("/");
            out.push(Pitch { hz, label });
        }
    }
    out.sort_by(|a, b| a.hz.partial_cmp(&b.hz).unwrap());
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bucket {
    pub hz: f64,
    /// Amplitude (0–255 byte scale) of the strongest peak in this bucket.
    pub weight: f64,
}

/// Find spectral peaks in `getByteFrequencyData`-style output.
///
/// A peak is a bin louder than both neighbours and above
/// [`BUCKET_MIN_AMPLITUDE`]. Its frequency is refined with parabolic
/// interpolation over the three bins (the bytes are linear in dB, which is
/// what that interpolation assumes), and plateaus of clipped bins are
/// centred. Only peaks within `min_hz..=max_hz` are reported, ascending.
pub fn spectral_peaks(
    bytes: &[u8],
    bin_hz: f64,
    hz_offset: f64,
    min_hz: f64,
    max_hz: f64,
    out: &mut Vec<(f64, f64)>,
) {
    if bytes.len() < 3 || bin_hz <= 0.0 {
        return;
    }
    let threshold = BUCKET_MIN_AMPLITUDE as u8;
    let mut k = 1;
    while k + 1 < bytes.len() {
        let b = bytes[k];
        if b <= threshold || b < bytes[k - 1] {
            k += 1;
            continue;
        }
        // extend over a plateau of equal values
        let mut end = k;
        while end + 1 < bytes.len() && bytes[end + 1] == b {
            end += 1;
        }
        if end + 1 >= bytes.len() || bytes[end + 1] >= b || bytes[k - 1] >= b {
            k = end + 1;
            continue;
        }
        let a = bytes[k - 1] as f64;
        let c = bytes[end + 1] as f64;
        let bf = b as f64;
        let centre = (k + end) as f64 / 2.0;
        let (position, height) = if end == k {
            let denom = a - 2.0 * bf + c;
            let delta = if denom != 0.0 {
                0.5 * (a - c) / denom
            } else {
                0.0
            };
            (centre + delta.clamp(-0.5, 0.5), bf - 0.25 * (a - c) * delta)
        } else {
            (centre, bf)
        };
        let hz = hz_offset + position * bin_hz;
        if hz >= min_hz && hz <= max_hz {
            out.push((hz, height));
        }
        k = end + 1;
    }
}

/// Merge peaks that lie within [`BUCKET_SENSITIVITY`] of each other, keeping
/// the loudest. `peaks` must be ascending in frequency.
pub fn overtone_buckets(peaks: &[(f64, f64)], buckets: &mut Vec<Bucket>) {
    buckets.clear();
    for &(hz, amplitude) in peaks {
        if amplitude <= BUCKET_MIN_AMPLITUDE {
            continue;
        }
        match buckets.last_mut() {
            Some(last) if last.hz / hz > BUCKET_SENSITIVITY => {
                if amplitude > last.weight {
                    last.hz = hz;
                    last.weight = amplitude;
                }
            }
            _ => buckets.push(Bucket {
                hz,
                weight: amplitude,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitchResult {
    /// Detected frequency (0 when nothing was detected).
    pub hz: f64,
    /// Frequency of the nearest labelled note (0 when none).
    pub target_hz: f64,
    /// Index into [`pitch_table`], or `None` for the empty label.
    pub note: Option<usize>,
}

/// Label `value` with the nearest note. `value <= 0` yields the empty result.
pub fn find_pitch_label(pitches: &[Pitch], value: f64) -> PitchResult {
    // first note at or above `value`
    let i = pitches.partition_point(|p| p.hz < value);
    let higher = i.min(pitches.len() - 1);
    let lower = i.checked_sub(1);

    let lower_hz = lower.map_or(0.0, |l| pitches[l].hz);
    let lower_diff = value - lower_hz;
    let higher_diff = pitches[higher].hz - value;

    if lower_diff < higher_diff {
        PitchResult {
            hz: value,
            target_hz: lower_hz,
            note: lower,
        }
    } else {
        PitchResult {
            hz: value,
            target_hz: pitches[higher].hz,
            note: Some(higher),
        }
    }
}

pub fn detect_pitch(pitches: &[Pitch], overtones: &[Bucket]) -> PitchResult {
    let mut previous_pitch = 0.0;
    let mut pitch = 0.0;

    // Buckets are ascending in frequency. The first bucket that is one
    // octave above its predecessor marks that predecessor as the fundamental.
    for overtone in overtones {
        let target = 0.5;
        let value = previous_pitch / overtone.hz;
        if value > target * PITCH_SENSITIVITY && value < target * (1.0 / PITCH_SENSITIVITY) {
            pitch = previous_pitch;
            break;
        }
        previous_pitch = overtone.hz;
    }

    // If it can't be found by overtones, just use the loudest pitch.
    let mut pitch_weight = 0.0;
    if previous_pitch != 0.0 && pitch == 0.0 {
        for overtone in overtones {
            if overtone.weight > pitch_weight {
                pitch = overtone.hz;
                pitch_weight = overtone.weight;
            }
        }
    }

    find_pitch_label(pitches, pitch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_starts_at_g_sharp_0_and_contains_a4() {
        let t = pitch_table();
        assert_eq!(t[0].label, "G#0/Ab0");
        assert!((t[0].hz - 25.9565).abs() < 1e-3);
        let a4 = t.iter().find(|p| p.label == "A4").unwrap();
        assert!((a4.hz - 440.0).abs() < 1e-9);
        assert!(t.windows(2).all(|w| w[0].hz < w[1].hz));
    }

    #[test]
    fn label_lookup_picks_the_nearest_note() {
        let t = pitch_table();
        for (hz, label) in [
            (441.0, "A4"),
            (439.5, "A4"),
            (465.0, "A#4/Bb4"),
            (452.0, "A4"),
            (454.0, "A#4/Bb4"),
        ] {
            let r = find_pitch_label(&t, hz);
            assert_eq!(t[r.note.unwrap()].label, label, "{hz} Hz");
        }
        let r = find_pitch_label(&t, 0.0);
        assert_eq!(r.note, None);
        assert_eq!(r.target_hz, 0.0);
        // above the table: last note
        let r = find_pitch_label(&t, 1e9);
        assert_eq!(r.note, Some(t.len() - 1));
    }

    #[test]
    fn pitch_from_overtones_prefers_octave_pairs() {
        let t = pitch_table();
        let mut buckets = Vec::new();
        overtone_buckets(
            &[(110.0, 150.0), (220.0, 120.0), (330.0, 110.0)],
            &mut buckets,
        );
        assert_eq!(buckets.len(), 3);
        let r = detect_pitch(&t, &buckets);
        assert!((r.hz - 110.0).abs() < 1e-9);
        assert_eq!(t[r.note.unwrap()].label, "A2");
    }

    #[test]
    fn nearby_peaks_merge_keeping_the_loudest() {
        let mut buckets = Vec::new();
        overtone_buckets(
            &[
                (100.0, 120.0),
                (105.0, 180.0),
                (108.0, 130.0),
                (200.0, 110.0),
            ],
            &mut buckets,
        );
        assert_eq!(buckets.len(), 2);
        assert_eq!(
            buckets[0],
            Bucket {
                hz: 105.0,
                weight: 180.0
            }
        );
    }

    #[test]
    fn quiet_rows_never_create_buckets() {
        let mut buckets = Vec::new();
        overtone_buckets(&[(100.0, 50.0), (200.0, 99.0)], &mut buckets);
        assert!(buckets.is_empty());
        let r = detect_pitch(&pitch_table(), &buckets);
        assert_eq!(r.hz, 0.0);
    }
}
