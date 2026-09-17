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
  /** Visible frequency range; omit for the scale's preset. */
  range?: FreqRange | null;
}

export interface FreqRange {
  minHz: number;
  maxHz: number;
}

export interface Marker {
  hz: number;
  label: string;
  /** Position from the bottom of the canvas, 0..1. */
  fraction: number;
}

/** Mirrors `EngineInfo` in Rust. */
export interface EngineInfo {
  height: number;
  fftSize: number;
  sampleRate: number;
  labelWidth: number;
  liveWidth: number;
  range: FreqRange;
  /** Highest first. */
  markers: Marker[];
  notes: string[];
}

export interface CaptureStatus {
  running: boolean;
  sampleRate: number;
  channels: number;
  device: string | null;
}
