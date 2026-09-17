import { atom } from "jotai";
import { atomWithStorage } from "jotai/utils";
import type {
  AnalyzerScale,
  ColoringMethod,
  FrequencyLabelMethod,
} from "@/engine/types";

export type { AnalyzerScale, ColoringMethod, FrequencyLabelMethod };

export const isRecordingAtom = atom(true);

export const fftSizeAtom = atomWithStorage("fft-size", 8192);

/**
 * Timeline canvas width in pixels. Set from the container's measured size
 * (times the device pixel ratio) so one second always occupies the same
 * screen distance, and wider screens show more time.
 */
export const timeseriesCanvasWidthAtom = atom(2048);

/**
 * Number of analysis rows. The original derived this from the FFT size and a
 * height factor that cancelled out to `546.13 * devicePixelRatio`.
 */
export const canvasHeightAtom = atom(() =>
  Math.floor(546.1333333333 * (window.devicePixelRatio || 1)),
);

export const coloringMethodAtom = atomWithStorage<ColoringMethod>(
  "coloring-method",
  "detailed",
);

export const frequencyLabelMethodAtom = atomWithStorage<FrequencyLabelMethod>(
  "frequency-labeling-method",
  "piano",
);

export const analyzerScaleAtom = atomWithStorage<AnalyzerScale>(
  "analyzer-scale",
  "piano",
);

/** Timeline scroll speed in CSS pixels per second of audio. */
export const TIMELINE_SCREEN_PIXELS_PER_SECOND = 60;
