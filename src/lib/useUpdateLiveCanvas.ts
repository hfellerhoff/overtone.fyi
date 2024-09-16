import { useAtomValue } from "jotai";
import { useCallback } from "react";
import { liveCanvasHeightAtom, liveCanvasWidthAtom } from "./fft";
import { IProcessedAudioData } from "./useUpdateAudioValues";

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

      const gradient = ctx.createLinearGradient(0, 0, 280, 0);
      gradient.addColorStop(0, "red");
      gradient.addColorStop(0.25, "orange");
      gradient.addColorStop(0.5, "yellow");
      gradient.addColorStop(0.75, "cyan");
      gradient.addColorStop(1, "darkblue");
      ctx.fillStyle = gradient;

      for (let i = 0; i < canvasHeight; i++) {
        const [, value] = hzData[i];

        if (!value) {
          continue;
        }

        ctx.fillRect(canvasWidth - value, canvasHeight - i, value, 1);
      }
    },
    [canvasHeight, canvasWidth]
  );
}
