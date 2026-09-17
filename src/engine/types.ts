export type ColoringMethod = "sigmoid" | "detailed";
export type FrequencyLabelMethod = "linear" | "piano";
export type AnalyzerScale = "piano" | "logarithmic";

/** Display settings owned by the UI; mirrors `DisplayConfig` in Rust. */
export interface DisplayConfig {
  fftSize: number;
  height: number;
  scale: AnalyzerScale;
  coloring: ColoringMethod;
  labeling: FrequencyLabelMethod;
}

/** Mirrors `EngineInfo` in Rust. */
export interface EngineInfo {
  height: number;
  fftSize: number;
  sampleRate: number;
  labelWidth: number;
  liveWidth: number;
  /** `[roundedHz, rowIndex]`, highest row first. */
  markers: [number, number][];
  notes: string[];
}

export interface CaptureStatus {
  running: boolean;
  sampleRate: number;
  channels: number;
  device: string | null;
}
