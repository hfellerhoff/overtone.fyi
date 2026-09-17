//! wasm-bindgen wrapper around [`overtone_core::Engine`].
//!
//! The browser feeds either raw PCM (from an `AudioWorklet`) with
//! [`WasmEngine::push_samples`], or a `getByteFrequencyData` result with
//! [`WasmEngine::frame_from_bytes`]. In both cases the returned packet is a
//! view into wasm memory (no copy) that stays valid until the next call.

use overtone_core::{Engine, EngineConfig};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmEngine {
    engine: Engine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// `config_json` is a JSON `EngineConfig` (camelCase keys).
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmEngine, JsError> {
        let config: EngineConfig =
            serde_json::from_str(config_json).map_err(|e| JsError::new(&e.to_string()))?;
        let engine = Engine::new(config).map_err(|e| JsError::new(&e))?;
        Ok(Self { engine })
    }

    pub fn configure(&mut self, config_json: &str) -> Result<(), JsError> {
        let config: EngineConfig =
            serde_json::from_str(config_json).map_err(|e| JsError::new(&e.to_string()))?;
        self.engine.configure(config).map_err(|e| JsError::new(&e))
    }

    /// JSON `EngineInfo`.
    pub fn info_json(&self) -> String {
        self.engine.info_json()
    }

    /// RGBA label strip (`labelWidth × height`), a view into wasm memory.
    pub fn label_strip(&self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view(self.engine.label_strip()) }
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        self.engine.push_mono(samples);
    }

    pub fn push_interleaved(&mut self, samples: &[f32], channels: usize) {
        self.engine.push_interleaved(samples, channels);
    }

    pub fn clear(&mut self) {
        self.engine.clear();
    }

    /// Analyse the pushed audio and return the frame packet (view into wasm
    /// memory; copy it if you need it past the next call).
    pub fn frame(&mut self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view(self.engine.frame()) }
    }

    /// Build a frame packet from browser `getByteFrequencyData` output.
    pub fn frame_from_bytes(&mut self, bytes: &[u8]) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view(self.engine.frame_from_bytes(bytes)) }
    }

    pub fn packet_len(&self) -> usize {
        overtone_core::engine::packet_len_for(self.engine.config().height)
    }
}
