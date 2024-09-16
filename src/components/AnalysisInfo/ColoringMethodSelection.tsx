import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { coloringMethodAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const COLORING_METHOD_OPTIONS = [
  {
    value: "detailed",
    label: "Detailed",
  },
  {
    value: "sigmoid",
    label: "Focused",
  },
] as const;

export default function ColoringMethodSelection() {
  const [coloringMethod, setColoringMethod] = useAtom(coloringMethodAtom);

  return (
    <AnalysisSquareWrapper label="Coloring">
      <div className="grid h-full grid-cols-1 pt-1 font-mono place-items-center">
        {COLORING_METHOD_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant={coloringMethod === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setColoringMethod(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
