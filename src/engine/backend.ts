import type { CaptureStatus, DisplayConfig, EngineInfo, ViewRequest } from "./types";

/**
 * An analysis backend owns audio capture and the Rust engine. The UI never
 * touches audio data: it asks for the next frame packet and blits it.
 */
export interface AnalysisBackend {
  readonly kind: "tauri" | "wasm";
  /** Start capturing audio (asks for microphone permission when needed). */
  start(): Promise<CaptureStatus>;
  stop(): Promise<CaptureStatus>;
  /** Apply display settings; resolves with the new engine info. */
  configure(config: DisplayConfig): Promise<EngineInfo>;
  /** RGBA label strip (`labelWidth × height`) for the current config. */
  labelStrip(): Promise<Uint8Array>;
  /** Next frame packet for `view`, or null when a frame is still in flight. */
  frame(view: ViewRequest): Promise<Uint8Array | null>;
  /** Forget all recorded audio and analysis. */
  clear(): Promise<void>;
  /** Release the microphone and free the engine. */
  dispose(): Promise<void>;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function createBackend(): Promise<AnalysisBackend> {
  if (isTauri()) {
    const { TauriBackend } = await import("./tauriBackend");
    return new TauriBackend();
  }
  const { WasmBackend } = await import("./wasmBackend");
  return WasmBackend.create();
}
