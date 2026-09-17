import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { frequencyLabelMethodAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const LABELING_METHOD_OPTIONS = [
  {
    value: "piano",
    label: "Piano",
  },
  {
    value: "linear",
    label: "Hz",
  },
] as const;

export default function FrequencyLabelingMethodSelection() {
  const [labelingMethod, setLabelingMethod] = useAtom(frequencyLabelMethodAtom);

  return (
    <AnalysisSquareWrapper label="Labeling Method">
      <div className="grid h-full grid-cols-1 pt-1 font-mono place-items-center">
        {LABELING_METHOD_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant={labelingMethod === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setLabelingMethod(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
