import { atom } from "jotai";
import { IProcessedAudioDataItem } from "./useUpdateAudioValues";

export const analyzerAtom = atom<AnalyserNode | null>(null);

export const isRecordingAtom = atom(true);

export const sampleRateAtom = atom(48000);
export const fftSizeAtom = atom(8192);
export const binSizeAtom = atom((get) => get(fftSizeAtom) * 2);
export const binSizeHzAtom = atom(
  (get) => get(sampleRateAtom) / get(binSizeAtom)
);

export const heightFactor = atom((get) => {
  return get(fftSizeAtom) / 546.1333333333;
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

// nyquist frequency
export const maxDisplayHzAtom = atom((get) => get(sampleRateAtom) / 2);

export const audioDataArrayAtom = atom(
  (get) => new Uint8Array(get(binSizeAtom))
);

export const frequencyMarkersAtom = atom<[hz: number, index: number][]>([]);
export const frequencyMarkerDistanceAtom = atom(25);

export const audioDataAnalysisAtom = atom<{
  highestAmplitudeValues: IProcessedAudioDataItem[];
}>({
  highestAmplitudeValues: [],
});
