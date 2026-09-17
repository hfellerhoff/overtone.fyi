import init, { WasmEngine } from "../wasm/pkg/overtone_wasm.js";
import type { AnalysisBackend } from "./backend";
import type { CaptureStatus, DisplayConfig, EngineInfo, ViewRequest } from "./types";
import { bandsForPreset } from "./bands";

const SAMPLE_RATE = 48000;

/** Translate the UI's display config into the engine's `EngineConfig`. */
function engineConfig(config: DisplayConfig, sampleRate: number) {
  const { bands, ...rest } = config;
  return { ...rest, sampleRate, bands: bandsForPreset(bands) };
}

/**
 * Browser backend: an AudioWorklet captures PCM and the Rust engine compiled
 * to WebAssembly does the FFT, mapping, colouring and pitch detection.
 */
export class WasmBackend implements AnalysisBackend {
  readonly kind = "wasm" as const;
  private engine: WasmEngine;
  private config: DisplayConfig;
  private audioContext: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private node: AudioWorkletNode | null = null;
  private pending: Float32Array[] = [];
  /** Serialises start/stop so React StrictMode's double effects cannot leak a stream. */
  private lifecycle: Promise<unknown> = Promise.resolve();

  private constructor(engine: WasmEngine, config: DisplayConfig) {
    this.engine = engine;
    this.config = config;
  }

  static async create(): Promise<WasmBackend> {
    await init();
    const config: DisplayConfig = {
      fftSize: 8192,
      height: 546,
      scale: "piano",
      coloring: "detailed",
      labeling: "piano",
      bands: "single",
    };
    const engine = new WasmEngine(JSON.stringify(engineConfig(config, SAMPLE_RATE)));
    return new WasmBackend(engine, config);
  }

  private status(): CaptureStatus {
    return {
      running: this.node !== null,
      sampleRate: this.audioContext?.sampleRate ?? SAMPLE_RATE,
      channels: 1,
      device: this.stream?.getAudioTracks()[0]?.label ?? null,
    };
  }

  start(): Promise<CaptureStatus> {
    const next = this.lifecycle.then(() => this.doStart());
    this.lifecycle = next.catch(() => undefined);
    return next;
  }

  stop(): Promise<CaptureStatus> {
    const next = this.lifecycle.then(() => this.doStop());
    this.lifecycle = next.catch(() => undefined);
    return next;
  }

  private async doStart(): Promise<CaptureStatus> {
    if (this.node) return this.status();
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const audioContext = new AudioContext({ sampleRate: SAMPLE_RATE });
    await audioContext.audioWorklet.addModule(
      new URL("./capture-worklet.js", import.meta.url),
    );
    const source = audioContext.createMediaStreamSource(stream);
    const node = new AudioWorkletNode(audioContext, "overtone-capture", {
      numberOfInputs: 1,
      numberOfOutputs: 0,
    });
    node.port.onmessage = (ev: MessageEvent<Float32Array>) => {
      this.pending.push(ev.data);
      // Never buffer more than the largest FFT window.
      while (this.pending.length > 64) this.pending.shift();
    };
    source.connect(node);
    await audioContext.resume();

    this.stream = stream;
    this.audioContext = audioContext;
    this.node = node;
    if (audioContext.sampleRate !== SAMPLE_RATE) {
      this.engine.configure(
        JSON.stringify(engineConfig(this.config, audioContext.sampleRate)),
      );
    }
    this.engine.clear();
    return this.status();
  }

  private async doStop(): Promise<CaptureStatus> {
    this.node?.disconnect();
    this.node = null;
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    await this.audioContext?.close();
    this.audioContext = null;
    this.pending.length = 0;
    return this.status();
  }

  async configure(config: DisplayConfig): Promise<EngineInfo> {
    this.config = config;
    this.engine.configure(
      JSON.stringify(engineConfig(config, this.status().sampleRate)),
    );
    return JSON.parse(this.engine.info_json()) as EngineInfo;
  }

  async labelStrip(): Promise<Uint8Array> {
    return this.engine.label_strip().slice();
  }

  async frame(view: ViewRequest): Promise<Uint8Array | null> {
    for (const chunk of this.pending) this.engine.push_samples(chunk);
    this.pending.length = 0;
    return this.engine.frame(JSON.stringify(view));
  }

  async dispose(): Promise<void> {
    await this.stop();
    this.engine.free();
  }
}
