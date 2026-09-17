//! Shared audio-analysis core used by both the desktop app (native, via cpal)
//! and the website (compiled to WebAssembly).
//!
//! The pipeline mirrors the original Web Audio implementation:
//!
//! 1. [`analyser`] — a re-implementation of `AnalyserNode.getByteFrequencyData`
//!    (Blackman window, real FFT, Chrome's magnitude scaling, dB → byte).
//! 2. [`mapping`] — maps every output pixel row to a frequency (piano or
//!    "logarithmic" scale) and to the FFT bins it reads from.
//! 3. [`color`] — amplitude → RGB for the scrolling spectrogram.
//! 4. [`labels`] — renders the piano-key / linear frequency label strip.
//! 5. [`pitch`] — overtone bucketing and pitch detection.
//! 6. [`engine`] — ties everything together and encodes binary packets that
//!    the frontend only has to blit.

pub mod analyser;
pub mod color;
pub mod engine;
pub mod labels;
pub mod mapping;
pub mod packet;
pub mod pitch;

pub use analyser::Analyser;
pub use color::Coloring;
pub use engine::{Engine, EngineConfig, LABEL_WIDTH, LIVE_WIDTH};
pub use labels::Labeling;
pub use mapping::Scale;
pub use pitch::PitchResult;
