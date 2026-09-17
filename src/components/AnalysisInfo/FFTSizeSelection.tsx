import { useAtom, useAtomValue } from "jotai";
import { Button } from "../ui/button";
import { bandPresetAtom, fftSizeAtom } from "@/lib/fft";
import { bandsForPreset, formatHz, formatWindow } from "@/engine/bands";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const FFT_SIZE_OPTIONS = [
  { value: 4096, label: "4k" },
  { value: 8192, label: "8k" },
  { value: 16384, label: "16k" },
  { value: 32768, label: "32k" },
];

export default function FFTSizeSelection() {
  const [fftSize, setFFTSize] = useAtom(fftSizeAtom);
  const preset = useAtomValue(bandPresetAtom);
  const bands = bandsForPreset(preset);

  const summary = bands
    .map((band, i) => {
      const from = i === 0 ? 0 : bands[i - 1].maxHz ?? 0;
      const to = band.maxHz === null ? "up" : `${formatHz(band.maxHz)}`;
      const range = band.maxHz === null ? `${formatHz(from)} Hz and up` : `${formatHz(from)}–${to} Hz`;
      return `${range}: ${formatWindow(fftSize / band.divisor)} window`;
    })
    .join(". ");

  return (
    <AnalysisSquareWrapper
      label="Resolution"
      tooltip={`The base window length in samples. Each frequency band uses a fixed fraction of it, so this scales every band together. Currently ${summary}.`}
      tooltipLink="https://en.wikipedia.org/wiki/Short-time_Fourier_transform#Resolution_issues"
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
