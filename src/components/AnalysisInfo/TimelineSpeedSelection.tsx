import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { timelineSpeedAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const TIMELINE_SPEED_OPTIONS = [
  { value: 1, label: "1x" },
  { value: 2, label: "2x" },
  { value: 4, label: "4x" },
  { value: 8, label: "8x" },
];

export default function TimelineSpeedSelection() {
  const [timelineSpeed, setTimelineSpeed] = useAtom(timelineSpeedAtom);

  return (
    <AnalysisSquareWrapper
      label="Timeline"
      tooltip="How fast the spectrogram scrolls. Higher values spread each moment across more pixels but keep less history on screen."
    >
      <div className="grid h-full grid-cols-2 pt-1 font-mono place-items-center">
        {TIMELINE_SPEED_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant={timelineSpeed === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setTimelineSpeed(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
