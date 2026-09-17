import PitchDisplay from "./PitchDisplay";
import FFTSizeSelection from "./FFTSizeSelection";
import ColoringMethodSelection from "./ColoringMethodSelection";
import FrequencyLabelingMethodSelection from "./FrequencyLabelingMethodSelection";
import AnalyzerScaleSelection from "./AnalyzerScaleSelection";
import TimelineSpeedSelection from "./TimelineSpeedSelection";

export default function AnalysisInfo() {
  return (
    <div className="flex h-full gap-2">
      <PitchDisplay />
      <ColoringMethodSelection />
      <FrequencyLabelingMethodSelection />
      <AnalyzerScaleSelection />
      <FFTSizeSelection />
      <TimelineSpeedSelection />
    </div>
  );
}
