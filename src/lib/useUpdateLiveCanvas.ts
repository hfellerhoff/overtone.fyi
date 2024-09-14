import { useAtomValue } from "jotai";
import { useCallback } from "react";
import { liveCanvasHeightAtom, liveCanvasWidthAtom } from "./fft";
import { IProcessedAudioData } from "./useUpdateAudioValues";
import { getAudioAmplitudeValueColor } from "./getHzDataElementColor";

export function useUpdateLiveCanvas() {
  const canvasHeight = useAtomValue(liveCanvasHeightAtom);
  const canvasWidth = useAtomValue(liveCanvasWidthAtom);

  return useCallback(
    async (canvas: HTMLCanvasElement, hzData: IProcessedAudioData) => {
      const ctx = canvas.getContext("2d", {
        willReadFrequently: true,
        alpha: false,
      });
      if (!ctx) return;

      ctx.fillStyle = "black";
      ctx.fillRect(0, 0, canvasWidth, canvasHeight);

      for (let i = 0; i < canvasHeight; i++) {
        const value = hzData[i][1];

        ctx.fillStyle = getAudioAmplitudeValueColor(value);
        ctx.fillRect(canvasWidth - value, canvasHeight - i, value, 1);
      }
    },
    [canvasHeight, canvasWidth]
  );
}
