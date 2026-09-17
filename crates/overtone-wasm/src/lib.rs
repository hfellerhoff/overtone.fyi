//! wasm-bindgen wrapper around [`overtone_core::Engine`].
//!
//! The browser feeds raw PCM from an `AudioWorklet` with
//! [`WasmEngine::push_samples`] and asks for frames with
//! [`WasmEngine::frame`]. The returned packet is a view into wasm memory
//! (no copy) that stays valid until the next call into the engine.

use overtone_core::{Engine, EngineConfig, ViewRequest};
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

    /// Analyse pending audio and render the view described by `view_json`
    /// (a JSON `ViewRequest`). Returns the frame packet as a view into wasm
    /// memory; copy it if you need it past the next call.
    pub fn frame(&mut self, view_json: &str) -> Result<js_sys::Uint8Array, JsError> {
        let view: ViewRequest =
            serde_json::from_str(view_json).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(unsafe { js_sys::Uint8Array::view(self.engine.frame(view)) })
    }
}
