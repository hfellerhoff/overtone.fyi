//! Binary layout of the per-frame packet handed to the frontend.
//!
//! All integers are little-endian. Layout (byte offsets):
//!
//! | offset | type | field |
//! |--------|------|-------|
//! | 0  | u32 | magic `0x4E54564F` ("OVTN") |
//! | 4  | u32 | version (2) |
//! | 8  | u32 | frame sequence number |
//! | 12 | u32 | height (rows) |
//! | 16 | u32 | flags (bit 0: new audio, bit 1: following live, bit 2: full redraw) |
//! | 20 | f32 | detected pitch (Hz, 0 = none) |
//! | 24 | f32 | nearest note frequency (Hz, 0 = none) |
//! | 28 | i32 | nearest note index into `info.notes` (-1 = none) |
//! | 32 | f32 | sample rate |
//! | 36 | i32 | shift: pixels to move the existing timeline right (negative = left) |
//! | 40 | u32 | columns: number of timeline columns included (== width for a full redraw) |
//! | 44 | u32 | width of the timeline in pixels |
//! | 64 | u32 | column_start: x position of the first included column |
//! | 48 | f32 | time at the left (newest) edge of the view, seconds |
//! | 52 | f32 | oldest recorded time, seconds |
//! | 56 | f32 | newest recorded time, seconds |
//! | 60 | f32 | timeline scale, pixels per second |
//! | 68 | u8 × 4·columns·height | timeline pixels, RGBA, row-major, x = 0 is newest |
//! | 68 + 4·columns·height | f32 × height | live bar length per row, index 0 = top row |

pub const MAGIC: u32 = 0x4E54_564F;
pub const VERSION: u32 = 2;
pub const HEADER_LEN: usize = 68;
pub const FLAG_NEW_AUDIO: u32 = 1;
pub const FLAG_LIVE: u32 = 2;
pub const FLAG_FULL: u32 = 4;

pub const fn columns_offset() -> usize {
    HEADER_LEN
}

pub const fn live_offset(columns: usize, height: usize) -> usize {
    HEADER_LEN + columns * height * 4
}

/// Length of a packet carrying `columns` timeline columns.
pub const fn packet_len(columns: usize, height: usize) -> usize {
    live_offset(columns, height) + height * 4
}

pub struct Header {
    pub seq: u32,
    pub height: u32,
    pub flags: u32,
    pub pitch_hz: f32,
    pub target_hz: f32,
    pub note: i32,
    pub sample_rate: f32,
    pub shift: i32,
    pub column_start: u32,
    pub columns: u32,
    pub width: u32,
    pub view_end: f32,
    pub history_start: f32,
    pub history_end: f32,
    pub px_per_second: f32,
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
        out[36..40].copy_from_slice(&self.shift.to_le_bytes());
        out[40..44].copy_from_slice(&self.columns.to_le_bytes());
        out[44..48].copy_from_slice(&self.width.to_le_bytes());
        out[48..52].copy_from_slice(&self.view_end.to_le_bytes());
        out[52..56].copy_from_slice(&self.history_start.to_le_bytes());
        out[56..60].copy_from_slice(&self.history_end.to_le_bytes());
        out[60..64].copy_from_slice(&self.px_per_second.to_le_bytes());
        out[64..68].copy_from_slice(&self.column_start.to_le_bytes());
    }
}
