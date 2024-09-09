import { atom } from "jotai";

export const analyzerAtom = atom<AnalyserNode | null>(null);

export const isRecordingAtom = atom(true);

export const sampleRateAtom = atom(48000);
export const fftSizeAtom = atom(8192);
export const binSizeAtom = atom((get) => get(fftSizeAtom) * 2);
export const binSizeHzAtom = atom(
  (get) => get(sampleRateAtom) / get(binSizeAtom)
);

export const timeseriesCanvasWidthAtom = atom((get) => get(fftSizeAtom) / 4);
export const timeseriesCanvasHeightAtom = atom((get) =>
  Math.floor(get(fftSizeAtom) / 12)
);

export const liveCanvasWidthAtom = atom(256 + 16);
export const liveCanvasHeightAtom = atom((get) =>
  Math.floor(get(fftSizeAtom) / 12)
);

// nyquist frequency
export const maxDisplayHzAtom = atom((get) => get(sampleRateAtom) / 2);

export const audioDataArrayAtom = atom(
  (get) => new Uint8Array(get(binSizeAtom))
);

export const frequencyMarkersAtom = atom<[hz: number, index: number][]>([]);
export const frequencyMarkerDistanceAtom = atom(25);
