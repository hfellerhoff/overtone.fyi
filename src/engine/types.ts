export type ColoringMethod = "sigmoid" | "detailed";
export type FrequencyLabelMethod = "linear" | "piano";
export type AnalyzerScale = "piano" | "logarithmic";
/** Relative window layout across frequency bands; mirrors `BandPreset`. */
export type BandPreset = "single" | "balanced" | "sharp";

/** Display settings owned by the UI; mirrors `DisplayConfig` in Rust. */
export interface DisplayConfig {
  fftSize: number;
  height: number;
  scale: AnalyzerScale;
  coloring: ColoringMethod;
  labeling: FrequencyLabelMethod;
  /** Visible frequency range; omit for the scale's preset. */
  range?: FreqRange | null;
  bands: BandPreset;
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
  ticksPerSecond: number;
  /** Seconds of history the memory budget can hold. */
  historySeconds: number;
  /** Seconds of raw audio retained for re-analysis after settings changes. */
  audioSeconds: number;
  /** `[minHz, maxHz, fftSize]` per band, ascending. */
  bands: [number, number, number][];
  notes: string[];
}

export interface CaptureStatus {
  running: boolean;
  sampleRate: number;
  channels: number;
  device: string | null;
}

/** Mirrors `ViewRequest` in Rust. */
export interface ViewRequest {
  /** Timeline width in pixels. */
  width: number;
  pxPerSecond: number;
  /** Newest edge of the timeline in seconds; null follows live. */
  viewEnd: number | null;
}
