import { invoke } from "@tauri-apps/api/core";
import type { AnalysisBackend } from "./backend";
import type { CaptureStatus, DisplayConfig, EngineInfo, ViewRequest } from "./types";

function toBytes(result: unknown): Uint8Array {
  if (result instanceof ArrayBuffer) return new Uint8Array(result);
  if (result instanceof Uint8Array) return result;
  if (Array.isArray(result)) return Uint8Array.from(result);
  throw new Error("unexpected binary IPC response");
}

/** Talks to the native engine in src-tauri via binary IPC. */
export class TauriBackend implements AnalysisBackend {
  readonly kind = "tauri" as const;
  private inFlight = false;

  async start(): Promise<CaptureStatus> {
    return invoke<CaptureStatus>("start_capture", { deviceId: null });
  }

  async stop(): Promise<CaptureStatus> {
    return invoke<CaptureStatus>("stop_capture");
  }

  async configure(config: DisplayConfig): Promise<EngineInfo> {
    return invoke<EngineInfo>("configure", { config });
  }

  async labelStrip(): Promise<Uint8Array> {
    return toBytes(await invoke("label_strip"));
  }

  async frame(view: ViewRequest): Promise<Uint8Array | null> {
    // Never queue up IPC calls if the previous frame has not returned yet.
    if (this.inFlight) return null;
    this.inFlight = true;
    try {
      return toBytes(await invoke("frame", { view }));
    } finally {
      this.inFlight = false;
    }
  }

  async dispose(): Promise<void> {
    await this.stop();
  }
}
