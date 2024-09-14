import { frequencyMarkersAtom } from "@/lib/fft";
import { useAtomValue } from "jotai";

interface IFrequencyMarkersProps {
  canvasHeight: number;
  canvas: HTMLCanvasElement | null;
}

export default function FrequencyMarkers({
  canvas,
  canvasHeight,
}: IFrequencyMarkersProps) {
  const frequencyMarkers = useAtomValue(frequencyMarkersAtom);

  const canvasHeightFactor = (canvas?.clientHeight ?? 0) / canvasHeight;

  return (
    <>
      {frequencyMarkers.map((marker) => (
        <div
          key={marker[0]}
          className="absolute right-0 flex items-center justify-center h-2.5 text-[10px] font-mono text-right select-none"
          style={{
            bottom: canvasHeightFactor * marker[1] - 5,
          }}
        >
          {marker[0]}hz
        </div>
      ))}
    </>
  );
}
