import { useAtomValue, useSetAtom } from "jotai";
import { useCallback } from "react";
import {
  coloringMethodAtom,
  frequencyMarkerDistanceAtom,
  frequencyMarkersAtom,
  timeseriesCanvasHeightAtom,
  timeseriesCanvasWidthAtom,
} from "./fft";
import { IProcessedAudioData } from "./useUpdateAudioValues";
import { getAudioAmplitudeValueColor } from "./getHzDataElementColor";

export function useUpdateTimeseriesCanvas() {
  const canvasHeight = useAtomValue(timeseriesCanvasHeightAtom);
  const canvasWidth = useAtomValue(timeseriesCanvasWidthAtom);
  const frequencyMarkerDistance = useAtomValue(frequencyMarkerDistanceAtom);
  const coloringMethod = useAtomValue(coloringMethodAtom);

  const setFrequencyMarkers = useSetAtom(frequencyMarkersAtom);

  return useCallback(
    async (canvas: HTMLCanvasElement, hzData: IProcessedAudioData) => {
      const ctx = canvas.getContext("2d", {
        willReadFrequently: true,
        alpha: false,
      });
      if (!ctx) return;

      const previousFrame = ctx.getImageData(0, 0, canvasWidth, canvasHeight);
      ctx.fillStyle = "black";
      ctx.fillRect(0, 0, canvasWidth, canvasHeight);
      ctx.putImageData(previousFrame, 1, 0);

      const updatedFrequencyMarkers: [hz: number, index: number][] = [];

      for (let i = 0; i < canvasHeight; i++) {
        const [startHz, value] = hzData[i];

        if (i % frequencyMarkerDistance === 0) {
          updatedFrequencyMarkers.push([Math.round(startHz), i]);
        }

        ctx.fillStyle = getAudioAmplitudeValueColor(
          startHz,
          value,
          coloringMethod
        );
        ctx.fillRect(0, canvasHeight - i, 1, 1);
      }

      updatedFrequencyMarkers.reverse();
      setFrequencyMarkers(updatedFrequencyMarkers);
    },
    [
      canvasHeight,
      canvasWidth,
      coloringMethod,
      frequencyMarkerDistance,
      setFrequencyMarkers,
    ]
  );
}
