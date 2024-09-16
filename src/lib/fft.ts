import { atom } from "jotai";
import {
  IProcessedAudioData,
  IProcessedAudioDataItem,
} from "./useUpdateAudioValues";
import { atomWithStorage } from "jotai/utils";

export const analyzerAtom = atom<AnalyserNode | null>(null);

export const isRecordingAtom = atom(true);

export const sampleRateAtom = atom(48000);
export const fftSizeAtom = atomWithStorage("fft-size", 8192);
export const binSizeAtom = atom((get) => get(fftSizeAtom) * 2);
export const binSizeHzAtom = atom(
  (get) => get(sampleRateAtom) / get(binSizeAtom)
);

console.log(devicePixelRatio);

export const heightFactor = atom((get) => {
  return get(fftSizeAtom) / (546.1333333333 * devicePixelRatio);
});

export const TIMESERIES_CANVAS_WIDTHS = {
  DESKTOP: 2048,
  MOBILE: 768,
};

export const timeseriesCanvasWidthAtom = atom(TIMESERIES_CANVAS_WIDTHS.DESKTOP);
export const timeseriesCanvasHeightAtom = atom((get) =>
  Math.floor(get(fftSizeAtom) / get(heightFactor))
);

export const liveCanvasWidthAtom = atom(256 + 16);
export const liveCanvasHeightAtom = atom((get) =>
  Math.floor(get(fftSizeAtom) / get(heightFactor))
);

export const frequencyLabelCanvasWidthAtom = atom(64);
export const frequencyLabelCanvasHeightAtom = atom((get) =>
  Math.floor(get(fftSizeAtom) / get(heightFactor))
);

// nyquist frequency
export const maxDisplayHzAtom = atom((get) => get(sampleRateAtom) / 2);

export const audioDataArrayAtom = atom(
  (get) => new Uint8Array(get(binSizeAtom))
);

export const audioDataHistoryAtom = atom<IProcessedAudioData[]>([]);

export const frequencyMarkersAtom = atom<[hz: number, index: number][]>([]);
export const frequencyMarkerDistanceAtom = atom(25);

export const audioDataAnalysisAtom = atom<{
  highestAmplitudeValues: IProcessedAudioDataItem[];
}>({
  highestAmplitudeValues: [],
});

export type ColoringMethod = "sigmoid" | "detailed";
export const coloringMethodAtom = atomWithStorage<ColoringMethod>(
  "coloring-method",
  "detailed"
);

export type FrequencyLabelMethod = "linear" | "piano";
export const frequencyLabelMethodAtom = atomWithStorage<FrequencyLabelMethod>(
  "frequency-labeling-method",
  "piano"
);

export type AnalayzerScale = "piano" | "logarithmic";
export const analyzerScaleAtom = atomWithStorage<AnalayzerScale>(
  "analyzer-scale",
  "piano"
);
