import type { Marker } from "@/engine/types";

interface IFrequencyMarkersProps {
  markers: Marker[];
}

export default function FrequencyMarkers({ markers }: IFrequencyMarkersProps) {
  return (
    <>
      {markers.map((marker) => (
        <div
          key={marker.hz}
          className="absolute right-0 flex items-center justify-center h-2.5 text-[10px] font-mono text-right select-none pointer-events-none"
          style={{
            bottom: `calc(${marker.fraction * 100}% - 5px)`,
          }}
        >
          {marker.label}hz
        </div>
      ))}
    </>
  );
}
