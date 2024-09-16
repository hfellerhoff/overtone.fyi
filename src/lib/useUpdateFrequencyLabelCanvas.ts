import { useAtomValue } from "jotai";
import { useCallback, useRef } from "react";
import {
  frequencyLabelCanvasHeightAtom,
  frequencyLabelCanvasWidthAtom,
  frequencyLabelMethodAtom,
} from "./fft";
import { IProcessedAudioData } from "./useUpdateAudioValues";
import { pitches } from "@/components/AnalysisInfo/pitches";

export function useUpdateFrequencyLabelCanvas() {
  const canvasHeight = useAtomValue(frequencyLabelCanvasHeightAtom);
  const canvasWidth = useAtomValue(frequencyLabelCanvasWidthAtom);

  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);

  const labelingMethod = useAtomValue(frequencyLabelMethodAtom);

  const framesRef = useRef(0);
  const secondsRef = useRef(0);

  const registerFrequencyLabelCanvas = useCallback(
    (canvas: HTMLCanvasElement | null) => {
      if (!canvas) return;

      ctxRef.current = canvas.getContext("2d", {
        willReadFrequently: true,
        alpha: false,
      });
    },
    []
  );

  const updateFrequencyLabelCanvas = useCallback(
    (time: DOMHighResTimeStamp, hzData: IProcessedAudioData) => {
      const ctx = ctxRef.current;
      if (!ctx) return;

      ctx.clearRect(0, 0, canvasWidth, canvasHeight);
      ctx.fillStyle = "white";

      let hundreds = 0;
      let thousands = 0;

      let pitchIndex = 0;

      ctx.font = "10px sans-serif";

      let key = "white";

      hzData.forEach((point, i) => {
        const [hz] = point;

        if (labelingMethod === "linear") {
          const pointHundreds = Math.floor(hz / 100);
          const pointThousands = Math.floor(hz / 1000);
          if (pointHundreds > hundreds) {
            hundreds = pointHundreds;

            ctx.fillStyle = "#444";
            ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
          } else if (pointThousands > thousands) {
            thousands = pointThousands;

            ctx.fillStyle = "#999";
            ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
          }
        } else if (labelingMethod === "piano" && !!pitches?.[pitchIndex]) {
          let pitch = pitches?.[pitchIndex];

          if (pitch[0] < hz) {
            while (pitch[0] < hz) {
              pitchIndex += 1;
              pitch = pitches?.[pitchIndex];
            }

            if (pitch) {
              const pitchLabel = pitch[1];
              if (pitchLabel.includes("C") && !pitchLabel.includes("#")) {
                ctx.fillStyle = "#8A8A8A";
                ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
                ctx.fillStyle = "#FFFFFF";
                key = "white";
              } else if (pitchLabel.includes("#")) {
                ctx.fillStyle = "#DDDDDD";
                ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
                ctx.fillStyle = "#111111";
                ctx.fillRect(0, canvasHeight - i, canvasWidth / 2, 1);
                key = "black";
              } else {
                ctx.fillStyle = "#DDDDDD";
                ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
                ctx.fillStyle = "#FFFFFF";
                key = "white";
              }
            }
          } else {
            if (key === "black") {
              ctx.fillRect(0, canvasHeight - i, canvasWidth / 2, 1);
              ctx.fillStyle = "#FFFFFF";
              ctx.fillRect(
                canvasWidth / 2,
                canvasHeight - i,
                canvasWidth / 2,
                1
              );
              ctx.fillStyle = "#111111";
            } else {
              ctx.fillRect(0, canvasHeight - i, canvasWidth, 1);
            }
          }
        }
      });

      framesRef.current += 1;

      const seconds = Math.floor(time / 1000);
      if (seconds > secondsRef.current) {
        secondsRef.current = seconds;
        // console.log(framesRef.current + " fps");
        framesRef.current = 0;
      }
    },
    [canvasHeight, canvasWidth, labelingMethod]
  );

  return {
    registerFrequencyLabelCanvas,
    updateFrequencyLabelCanvas,
  };
}
