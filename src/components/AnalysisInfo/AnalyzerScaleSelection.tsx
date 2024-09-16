import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { analyzerScaleAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const ANALYZER_SCALE_OPTIONS = [
  {
    value: "piano",
    label: "Piano",
  },
  {
    value: "logarithmic",
    label: "Log",
  },
] as const;

export default function AnalyzerScaleSelection() {
  const [analyzerScale, setAnalyzerScale] = useAtom(analyzerScaleAtom);

  return (
    <AnalysisSquareWrapper label="Frequency Scale">
      <div className="grid h-full grid-cols-1 pt-1 font-mono place-items-center">
        {ANALYZER_SCALE_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant={analyzerScale === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setAnalyzerScale(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
