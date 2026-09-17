import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { bandPresetAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const BAND_OPTIONS = [
  { value: "single", label: "Single" },
  { value: "balanced", label: "Balanced" },
  { value: "sharp", label: "Sharp" },
] as const;

export default function BandsSelection() {
  const [preset, setPreset] = useAtom(bandPresetAtom);

  return (
    <AnalysisSquareWrapper
      label="Bands"
      tooltip="How the window length varies across frequency. Single uses one window everywhere. Balanced halves it every octave above 250 Hz, so the treble responds faster while the bass keeps its pitch detail. Sharp starts halving at 125 Hz and goes further, trading some pitch precision in the treble for the fastest response."
    >
      <div className="flex flex-col items-center justify-center h-full gap-0.5 font-mono">
        {BAND_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            className="h-6 px-2"
            variant={preset === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setPreset(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
