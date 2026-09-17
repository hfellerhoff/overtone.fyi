interface IFrequencyMarkersProps {
  canvasHeight: number;
  canvas: HTMLCanvasElement | null;
  /** `[hz, rowIndex]` pairs from the engine. */
  markers: [number, number][];
}

export default function FrequencyMarkers({
  canvas,
  canvasHeight,
  markers,
}: IFrequencyMarkersProps) {
  const canvasHeightFactor = (canvas?.clientHeight ?? 0) / canvasHeight;

  return (
    <>
      {markers.map((marker) => (
        <div
          key={marker[1]}
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
