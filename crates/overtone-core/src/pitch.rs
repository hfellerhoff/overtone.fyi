//! Overtone bucketing and pitch detection. Ports of `getOvertoneBuckets`,
//! `getPitch` and `findPitchLabel`, plus the generated `pitches` table.

use crate::mapping::pitch_by_number;

/// The percent similarity where two distinct Hz values are lumped into one bucket.
pub const BUCKET_SENSITIVITY: f64 = 0.9;
pub const BUCKET_MIN_AMPLITUDE: f64 = 100.0;
pub const PITCH_SENSITIVITY: f64 = 0.85;
/// How many of the loudest rows feed the pitch detector.
pub const HIGHEST_AMPLITUDE_COUNT: usize = 128;

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
    /// The cumulative amplitude values used to calculate this bucket.
    pub weight: f64,
}

/// `data` must be ordered by ascending Hz, like `highestAmplitudeValues`.
pub fn overtone_buckets(data: &[(f64, f64)], buckets: &mut Vec<Bucket>) {
    buckets.clear();
    for &(item_hz, item_amplitude) in data {
        match buckets
            .iter()
            .position(|bucket| bucket.hz / item_hz > BUCKET_SENSITIVITY)
        {
            None => {
                if item_amplitude > BUCKET_MIN_AMPLITUDE {
                    buckets.push(Bucket {
                        hz: item_hz,
                        weight: item_amplitude,
                    });
                }
            }
            Some(index) => {
                let bucket = &mut buckets[index];
                let updated_weight = bucket.weight + item_amplitude;
                let ratio = item_amplitude / updated_weight;
                let updated_hz = item_hz * ratio + bucket.hz * (1.0 - ratio);
                bucket.hz = updated_hz;
                bucket.weight = updated_weight;
            }
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

pub fn find_pitch_label(pitches: &[Pitch], value: f64) -> PitchResult {
    let mut lower: Option<usize> = None;
    let mut i = 0usize;
    let mut entry = &pitches[i];
    while entry.hz < value {
        lower = Some(i);
        if i + 1 >= pitches.len() {
            break;
        }
        i += 1;
        entry = &pitches[i];
    }
    let higher = if i + 1 < pitches.len() { i + 1 } else { i };

    let lower_hz = lower.map(|l| pitches[l].hz).unwrap_or(0.0);
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

    for overtone in overtones {
        let target = 0.5;
        let value = previous_pitch / overtone.hz;
        if value > target * PITCH_SENSITIVITY && value < target * (1.0 / PITCH_SENSITIVITY) {
            pitch = overtone.hz;
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
    fn label_lookup_matches_original_bias() {
        let t = pitch_table();
        let r = find_pitch_label(&t, 441.0);
        assert_eq!(t[r.note.unwrap()].label, "A4");
        // 465 Hz is nearer to A#4 (466.16) but the original compares against
        // the note after the first one >= value, so it still reports A4.
        let r = find_pitch_label(&t, 465.0);
        assert_eq!(t[r.note.unwrap()].label, "A4");
        let r = find_pitch_label(&t, 0.0);
        assert_eq!(r.note, None);
        assert_eq!(r.target_hz, 0.0);
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
        assert!((r.hz - 220.0).abs() < 1e-9);
        assert_eq!(t[r.note.unwrap()].label, "A3");
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
