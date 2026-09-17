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

export const TIMESERIES_CANVAS_WIDTHS = {
  DESKTOP: 2048,
  MOBILE: 768,
};

export const timeseriesCanvasWidthAtom = atom(TIMESERIES_CANVAS_WIDTHS.DESKTOP);

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

/**
 * Timeline scroll speed multiplier. 1x scrolls 60 px/s, so the full-width
 * desktop timeline (2048 px) holds about 34 s of history; 2x holds ~17 s.
 */
export const TIMELINE_BASE_PIXELS_PER_SECOND = 60;
export const timelineSpeedAtom = atomWithStorage("timeline-speed", 2);
