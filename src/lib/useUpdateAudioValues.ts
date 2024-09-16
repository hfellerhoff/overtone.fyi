import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { useCallback } from "react";
import {
  audioDataAnalysisAtom,
  audioDataArrayAtom,
  audioDataHistory,
  audioDataHistoryAtom,
  binSizeHzAtom,
  maxDisplayHzAtom,
  timeseriesCanvasHeightAtom,
} from "./fft";

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
  maxDisplayHz: number
): IProcessedAudioData {
  return Array(canvasHeight)
    .fill(0)
    .map((_, i) => {
      const logarithmicPixelSize = getLogarithmicPixelSize(
        maxDisplayHz,
        canvasHeight,
        i
      );
      const pixelSize = logarithmicPixelSize;

      const logarithmicPixelStart = getLogarithmicPixelStart(
        maxDisplayHz,
        canvasHeight,
        i
      );
      const pixelStartFrequency = logarithmicPixelStart;

      const pixelFrequencyMidpoint = pixelStartFrequency + pixelSize / 2;

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

      return [logarithmicPixelStart, value];
    });
}

export function useUpdateAudioValues() {
  const canvasHeight = useAtomValue(timeseriesCanvasHeightAtom);
  const binSizeHz = useAtomValue(binSizeHzAtom);
  const maxDisplayHz = useAtomValue(maxDisplayHzAtom);
  const setAudioAnalysis = useSetAtom(audioDataAnalysisAtom);

  const audioDataArray = useAtomValue(audioDataArrayAtom);
  const audioDataHistory = useAtomValue(audioDataHistoryAtom);

  return useCallback(
    (analyzer: AnalyserNode) => {
      analyzer.getByteFrequencyData(audioDataArray);

      const hzDataArray = getHzDataArray(
        audioDataArray,
        canvasHeight,
        binSizeHz,
        maxDisplayHz
      );

      const highestAmplitudeValues = hzDataArray.reduce(
        (accValues, element) => {
          if (accValues.length < 64) {
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

      return hzDataArray;
    },
    [audioDataArray, binSizeHz, canvasHeight, maxDisplayHz, setAudioAnalysis]
  );
}
