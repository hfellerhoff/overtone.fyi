//! Desktop backend: captures microphone audio with cpal on a realtime thread,
//! hands the samples to the shared [`overtone_core::Engine`] through a
//! lock-free ring buffer, and exposes the analysis to the webview as binary
//! IPC responses. The webview only blits what it receives.

pub mod capture;

use capture::{Capture, DeviceInfo};
use overtone_core::engine::{EngineConfig, EngineInfo};
use overtone_core::{Coloring, Engine, Labeling, Scale};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::ipc::Response;
use tauri::State;

/// The display-related part of [`EngineConfig`]; the sample rate is owned by
/// the capture device, not the frontend.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayConfig {
    pub fft_size: usize,
    pub height: usize,
    pub scale: Scale,
    pub coloring: Coloring,
    pub labeling: Labeling,
}

impl DisplayConfig {
    fn into_engine_config(self, sample_rate: f64) -> EngineConfig {
        EngineConfig {
            fft_size: self.fft_size,
            sample_rate,
            height: self.height,
            scale: self.scale,
            coloring: self.coloring,
            labeling: self.labeling,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub running: bool,
    pub sample_rate: f64,
    pub channels: u16,
    pub device: Option<String>,
}

pub struct AppState {
    engine: Mutex<Engine>,
    capture: Mutex<Option<Capture>>,
    /// Scratch buffer for draining the capture ring into the engine.
    drain: Mutex<Vec<f32>>,
    /// Set `OVERTONE_LOG=1` to print frame statistics to stderr.
    log: bool,
    frames: AtomicU64,
    samples: AtomicU64,
}

impl AppState {
    fn new() -> Self {
        Self {
            engine: Mutex::new(Engine::new(EngineConfig::default()).expect("default config")),
            capture: Mutex::new(None),
            drain: Mutex::new(vec![0.0; 1 << 16]),
            log: std::env::var_os("OVERTONE_LOG").is_some(),
            frames: AtomicU64::new(0),
            samples: AtomicU64::new(0),
        }
    }

    fn status(&self, capture: &Option<Capture>) -> CaptureStatus {
        match capture {
            Some(c) => CaptureStatus {
                running: true,
                sample_rate: c.sample_rate() as f64,
                channels: c.channels(),
                device: Some(c.device_name().to_owned()),
            },
            None => CaptureStatus {
                running: false,
                sample_rate: self.engine.lock().config().sample_rate,
                channels: 0,
                device: None,
            },
        }
    }
}

#[tauri::command(async)]
fn list_input_devices() -> Result<Vec<DeviceInfo>, String> {
    capture::list_input_devices()
}

/// Start (or restart) microphone capture. Returns the capture status; the
/// engine is reconfigured to the device's real sample rate.
#[tauri::command(async)]
fn start_capture(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> Result<CaptureStatus, String> {
    let mut capture = state.capture.lock();
    *capture = None;
    let new = Capture::start(device_id.as_deref())?;
    {
        let mut engine = state.engine.lock();
        let mut config = engine.config().clone();
        if config.sample_rate != new.sample_rate() as f64 {
            config.sample_rate = new.sample_rate() as f64;
            engine.configure(config)?;
        }
        engine.clear();
    }
    *capture = Some(new);
    Ok(state.status(&capture))
}

#[tauri::command(async)]
fn stop_capture(state: State<'_, AppState>) -> CaptureStatus {
    let mut capture = state.capture.lock();
    *capture = None;
    state.status(&capture)
}

#[tauri::command]
fn capture_status(state: State<'_, AppState>) -> CaptureStatus {
    state.status(&state.capture.lock())
}

/// Apply display settings. Returns the new [`EngineInfo`].
#[tauri::command]
fn configure(state: State<'_, AppState>, config: DisplayConfig) -> Result<EngineInfo, String> {
    let mut engine = state.engine.lock();
    let sample_rate = engine.config().sample_rate;
    engine.configure(config.into_engine_config(sample_rate))?;
    Ok(engine.info())
}

#[tauri::command]
fn engine_info(state: State<'_, AppState>) -> EngineInfo {
    state.engine.lock().info()
}

/// RGBA label strip for the current configuration (`labelWidth × height`).
#[tauri::command(async)]
fn label_strip(state: State<'_, AppState>) -> Response {
    Response::new(state.engine.lock().label_strip().to_vec())
}

/// Drain captured audio into the engine and return the next frame packet
/// (see `overtone_core::packet` for the layout).
#[tauri::command(async)]
fn frame(state: State<'_, AppState>) -> Response {
    let started = std::time::Instant::now();
    let mut engine = state.engine.lock();
    let mut drained = 0u64;
    {
        let mut capture = state.capture.lock();
        if let Some(c) = capture.as_mut() {
            let mut drain = state.drain.lock();
            loop {
                let n = c.drain(&mut drain);
                if n == 0 {
                    break;
                }
                drained += n as u64;
                engine.push_mono(&drain[..n]);
            }
        }
    }
    let packet = engine.frame().to_vec();
    if state.log {
        let frames = state.frames.fetch_add(1, Ordering::Relaxed) + 1;
        let samples = state.samples.fetch_add(drained, Ordering::Relaxed) + drained;
        if frames.is_multiple_of(60) {
            let pitch = engine.last_pitch();
            eprintln!(
                "[overtone] frame {frames}: {samples} samples so far, {} bytes, {:?}, pitch {:.1} Hz",
                packet.len(),
                started.elapsed(),
                pitch.hz
            );
        }
    }
    Response::new(packet)
}

/// Register state and commands on a builder (shared by `run` and tests).
pub fn build_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            list_input_devices,
            start_capture,
            stop_capture,
            capture_status,
            configure,
            engine_info,
            label_strip,
            frame,
        ])
}

pub fn run() {
    build_app(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running overtone");
}
