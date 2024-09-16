import { audioDataAnalysisAtom } from "@/lib/fft";
import { IProcessedAudioData } from "@/lib/useUpdateAudioValues";
import { useAtomValue } from "jotai";
import { useMemo } from "react";
import PitchDisplay from "./PitchDisplay";
import FFTSizeSelection from "./FFTSizeSelection";
import ColoringMethodSelection from "./ColoringMethodSelection";
import FrequencyLabelingMethodSelection from "./FrequencyLabelingMethodSelection";
import AnalyzerScaleSelection from "./AnalyzerScaleSelection";

// The percent similarity where two distinct HZ values
// will be lumped into one bucket
const BUCKET_SENSITIVITY = 0.9;

// TODO: Update to be dynamic based on highest HZ value
const BUCKET_MIN_AMPLITUDE = 100;

export type OvertoneBucket = {
  hz: number;
  /**
   * The cumulative amplitude values used to calculate this bucket.
   */
  weight: number;
};

function getOvertoneBuckets(data: IProcessedAudioData): OvertoneBucket[] {
  const buckets: OvertoneBucket[] = [];

  data.forEach((item) => {
    const itemHz = item[0];
    const itemAmplitude = item[1];

    const bucketIndex = buckets.findIndex(
      (bucket) => bucket.hz / itemHz > BUCKET_SENSITIVITY
    );

    if (bucketIndex < 0) {
      if (itemAmplitude > BUCKET_MIN_AMPLITUDE) {
        buckets.push({
          hz: itemHz,
          weight: itemAmplitude,
        });
      }
      return;
    }

    const bucket = buckets[bucketIndex];

    // simplistic method
    // if (itemAmplitude > bucket.weight) {
    //   bucket.hz = itemHz;
    //   bucket.weight = itemAmplitude;
    // }

    // averaging method
    const updatedWeight = bucket.weight + itemAmplitude;
    const ratio = itemAmplitude / updatedWeight;
    const updatedHz = itemHz * ratio + bucket.hz * (1 - ratio);

    bucket.hz = updatedHz;
    bucket.weight = updatedWeight;
  });

  return buckets;
}

export default function AnalysisInfo() {
  const data = useAtomValue(audioDataAnalysisAtom);

  const overtoneBuckets = useMemo(
    () => getOvertoneBuckets(data.highestAmplitudeValues),
    [data.highestAmplitudeValues]
  );

  return (
    <div className="flex h-full gap-2">
      <PitchDisplay overtoneBuckets={overtoneBuckets} />
      <ColoringMethodSelection />
      <FrequencyLabelingMethodSelection />
      <AnalyzerScaleSelection />
      <FFTSizeSelection />
      {/* <div className="grid h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background">
        {overtoneBuckets.map((bucket) => (
          <div>{Math.round(bucket.hz)}hz</div>
        ))}
      </div> */}
    </div>
  );
}
