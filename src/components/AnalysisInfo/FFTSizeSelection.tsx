import { useAtom } from "jotai";
import { Button } from "../ui/button";
import { fftSizeAtom } from "@/lib/fft";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const FFT_SIZE_OPTIONS = [
  { value: 4096, label: "4k" },
  { value: 8192, label: "8k" },
  { value: 16384, label: "16k" },
  { value: 32768, label: "32k" },
];

export default function FFTSizeSelection() {
  const [fftSize, setFFTSize] = useAtom(fftSizeAtom);

  return (
    <AnalysisSquareWrapper
      label="Resolution"
      tooltip="The window size, in samples, that is used when performing a Fast Fourier Transform. Larger windows separate close pitches better but respond more slowly."
      tooltipLink="https://en.wikipedia.org/wiki/Fast_Fourier_transform"
    >
      <div className="grid h-full grid-cols-2 pt-1 font-mono place-items-center">
        {FFT_SIZE_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant={fftSize === option.value ? "secondary" : "ghost"}
            onClick={() => {
              setFFTSize(option.value);
            }}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </AnalysisSquareWrapper>
  );
}
