//! Binary layout of the per-frame packet handed to the frontend.
//!
//! All integers are little-endian. Layout (byte offsets):
//!
//! | offset | type | field |
//! |--------|------|-------|
//! | 0  | u32 | magic `0x4E54564F` ("OVTN") |
//! | 4  | u32 | version |
//! | 8  | u32 | frame sequence number |
//! | 12 | u32 | height (rows) |
//! | 16 | u32 | flags (bit 0: new audio since previous frame) |
//! | 20 | f32 | detected pitch (Hz, 0 = none) |
//! | 24 | f32 | nearest note frequency (Hz, 0 = none) |
//! | 28 | i32 | nearest note index into `info.notes` (-1 = none) |
//! | 32 | f32 | sample rate |
//! | 36 | u32 | reserved |
//! | 40 | u8 × 4·height | new spectrogram column, RGBA, index 0 = top row |
//! | 40 + 4·height | f32 × height | live bar length per row, index 0 = top row |

pub const MAGIC: u32 = 0x4E54_564F;
pub const VERSION: u32 = 1;
pub const HEADER_LEN: usize = 40;
pub const FLAG_NEW_AUDIO: u32 = 1;

pub const fn column_offset() -> usize {
    HEADER_LEN
}

pub const fn live_offset(height: usize) -> usize {
    HEADER_LEN + height * 4
}

pub const fn packet_len(height: usize) -> usize {
    HEADER_LEN + height * 8
}

pub struct Header {
    pub seq: u32,
    pub height: u32,
    pub flags: u32,
    pub pitch_hz: f32,
    pub target_hz: f32,
    pub note: i32,
    pub sample_rate: f32,
}

impl Header {
    pub fn write(&self, out: &mut [u8]) {
        debug_assert!(out.len() >= HEADER_LEN);
        out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        out[4..8].copy_from_slice(&VERSION.to_le_bytes());
        out[8..12].copy_from_slice(&self.seq.to_le_bytes());
        out[12..16].copy_from_slice(&self.height.to_le_bytes());
        out[16..20].copy_from_slice(&self.flags.to_le_bytes());
        out[20..24].copy_from_slice(&self.pitch_hz.to_le_bytes());
        out[24..28].copy_from_slice(&self.target_hz.to_le_bytes());
        out[28..32].copy_from_slice(&self.note.to_le_bytes());
        out[32..36].copy_from_slice(&self.sample_rate.to_le_bytes());
        out[36..40].copy_from_slice(&0u32.to_le_bytes());
    }
}
