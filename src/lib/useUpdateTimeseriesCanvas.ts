import { useAtomValue, useSetAtom } from "jotai";
import { useCallback, useRef } from "react";
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

  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);

  // const framesRef = useRef(0);
  // const secondsRef = useRef(0);

  const registerTimeseriesCanvas = useCallback(
    (canvas: HTMLCanvasElement | null) => {
      if (!canvas) return;

      ctxRef.current = canvas.getContext("2d", {
        willReadFrequently: true,
        alpha: false,
      });
    },
    []
  );

  const updateTimeseriesCanvas = useCallback(
    (_time: DOMHighResTimeStamp, hzData: IProcessedAudioData) => {
      const ctx = ctxRef.current;
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

        if (!value) {
          continue;
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

      // framesRef.current += 1;

      // const seconds = Math.floor(time / 1000);
      // if (seconds > secondsRef.current) {
      //   secondsRef.current = seconds;
      //   console.log(framesRef.current + " fps");
      //   framesRef.current = 0;
      // }
    },
    [
      canvasHeight,
      canvasWidth,
      coloringMethod,
      frequencyMarkerDistance,
      setFrequencyMarkers,
    ]
  );

  return {
    registerTimeseriesCanvas,
    updateTimeseriesCanvas,
  };
}
