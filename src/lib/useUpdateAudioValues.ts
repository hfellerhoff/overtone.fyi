import { useAtomValue, useSetAtom } from "jotai";
import { useCallback } from "react";
import {
  AnalayzerScale,
  analyzerScaleAtom,
  audioDataAnalysisAtom,
  audioDataArrayAtom,
  audioDataHistoryAtom,
  binSizeHzAtom,
  maxDisplayHzAtom,
  timeseriesCanvasHeightAtom,
} from "./fft";

const factor = Math.pow(2, 1 / 12);

const referenceHz = 440;
const referenceNoteNumber = 49;
const notes = 84;
const baseNote = 12;

function getPianoPixelStart(pixelIndex: number, pixelCount: number) {
  const notePixelDistance = pixelCount / notes;
  const adjustedPixelIndex = baseNote * notePixelDistance + pixelIndex;
  // const computedFactor = notePixelDistance * factor;

  return (
    referenceHz *
    Math.pow(
      factor,
      adjustedPixelIndex / notePixelDistance - referenceNoteNumber
    )
  );
}

function getPianoPixelSize(pixelIndex: number, pixelCount: number) {
  const curr = getPianoPixelStart(pixelIndex, pixelCount);
  const next = getPianoPixelStart(pixelIndex + 1, pixelCount);
  return next - curr;
}

const getLogarithmicPixelStart = (
  maxHz: number,
  pixelCount: number,
  pixelNumber: number
) => {
  return (pixelCount / maxHz) * Math.pow(pixelNumber, 2);
};

const getLogarithmicPixelSize = (
  maxHz: number,
  pixelCount: number,
  pixelNumber: number
) => {
  const curr = getLogarithmicPixelStart(maxHz, pixelCount, pixelNumber);
  const next = getLogarithmicPixelStart(maxHz, pixelCount, pixelNumber + 1);
  return next - curr;
};

export type IProcessedAudioDataItem = [pixelStart: number, value: number];
export type IProcessedAudioData = IProcessedAudioDataItem[];

function getHzDataArray(
  audioDataArray: Uint8Array,
  canvasHeight: number,
  binSizeHz: number,
  maxDisplayHz: number,
  analyzerScale: AnalayzerScale
): IProcessedAudioData {
  return Array(canvasHeight)
    .fill(0)
    .map((_, i) => {
      let pixelSize = 0;
      let pixelStartHz = 0;

      if (analyzerScale === "logarithmic") {
        pixelSize = getLogarithmicPixelSize(maxDisplayHz, canvasHeight, i);
        pixelStartHz = getLogarithmicPixelStart(maxDisplayHz, canvasHeight, i);
      } else {
        pixelSize = getPianoPixelSize(i, canvasHeight);
        pixelStartHz = getPianoPixelStart(i, canvasHeight);
      }

      const pixelFrequencyMidpoint = pixelStartHz + pixelSize / 2;

      let j = 0;
      let previousDistance = 100000000;

      while (
        previousDistance > Math.abs(pixelFrequencyMidpoint - j * binSizeHz)
      ) {
        previousDistance = Math.abs(pixelFrequencyMidpoint - j * binSizeHz);
        j++;
      }

      // average the close bins to get the most accurate frequency value
      const lowerBinDistance = Math.abs(
        pixelFrequencyMidpoint - (j - 1) * binSizeHz
      );
      const upperBinDistance = Math.abs(pixelFrequencyMidpoint - j * binSizeHz);

      const ratio = lowerBinDistance / upperBinDistance;

      let value = 0;
      if (j + 1 <= audioDataArray.length) {
        const lowerBinValue = audioDataArray[j - 1] / 1 + ratio;
        const upperBinValue = audioDataArray[j] / 1 - ratio;
        value = (lowerBinValue + upperBinValue) / 2;
      }

      return [pixelStartHz, value];
    });
}

export function useUpdateAudioValues() {
  const canvasHeight = useAtomValue(timeseriesCanvasHeightAtom);
  const binSizeHz = useAtomValue(binSizeHzAtom);
  const maxDisplayHz = useAtomValue(maxDisplayHzAtom);
  const setAudioAnalysis = useSetAtom(audioDataAnalysisAtom);

  const audioDataArray = useAtomValue(audioDataArrayAtom);
  const audioDataHistory = useAtomValue(audioDataHistoryAtom);
  const analyzerScale = useAtomValue(analyzerScaleAtom);

  return useCallback(
    (analyzer: AnalyserNode) => {
      analyzer.getByteFrequencyData(audioDataArray);

      const hzDataArray = getHzDataArray(
        audioDataArray,
        canvasHeight,
        binSizeHz,
        maxDisplayHz,
        analyzerScale
      );

      const highestAmplitudeValues = hzDataArray.reduce(
        (accValues, element) => {
          if (accValues.length < 128) {
            accValues.push(element);
            return accValues;
          }

          if (accValues.some((v) => v[1] < element[1])) {
            accValues.push(element);
            accValues.sort((a, b) => (a[1] > b[1] ? -1 : 1));
            accValues.pop();
          }

          return accValues;
        },
        [] as IProcessedAudioDataItem[]
      );

      // Order by hz
      highestAmplitudeValues.sort((a, b) => (a[0] < b[0] ? -1 : 1));

      setAudioAnalysis({
        highestAmplitudeValues,
      });

      audioDataHistory.unshift(hzDataArray);

      if (audioDataHistory.length > 8192) {
        audioDataHistory.pop();
      }

      return hzDataArray;
    },
    [
      analyzerScale,
      audioDataArray,
      audioDataHistory,
      binSizeHz,
      canvasHeight,
      maxDisplayHz,
      setAudioAnalysis,
    ]
  );
}
