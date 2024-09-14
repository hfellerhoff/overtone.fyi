import { useMemo } from "react";
import type { OvertoneBucket } from "./AnalysisInfo";
import { pitches } from "./pitches";

const PITCH_SENSITIVITY = 0.8;

type PitchAndLabel = [hz: number, label: string];

const entries = Object.entries(pitches)
  .map((entry) => [parseFloat(entry[0]), entry[1]] as PitchAndLabel)
  .sort((a, b) => (a[0] < b[0] ? -1 : 1));
function findPitchLabel(value: number) {
  let lowerValue: PitchAndLabel = [0, ""];
  let higherValue: PitchAndLabel = [0, ""];

  let i = 0;

  let entry = entries[i];

  while (entry[0] < value) {
    lowerValue = entry;
    i += 1;
    entry = entries[i];
  }

  higherValue = entries[i + 1];

  const lowerDiff = value - lowerValue[0];
  const higherDiff = higherValue[0] - value;

  if (lowerDiff < higherDiff) {
    return {
      value,
      targetValue: lowerValue[0],
      label: lowerValue[1],
    };
  }

  return {
    value,
    targetValue: higherValue[0],
    label: higherValue[1],
  };
}

function getPitch(overtones: OvertoneBucket[]) {
  let previousPitch = 0;
  let pitch = 0;

  for (let i = 0; i < overtones.length; i += 1) {
    const overtone = overtones[i];
    const target = 0.5;
    const value = previousPitch / overtone.hz;

    if (
      value > target * PITCH_SENSITIVITY &&
      value < target * (1 / PITCH_SENSITIVITY)
    ) {
      pitch = overtone.hz;
      break;
    }

    previousPitch = overtone.hz;
  }

  if (!!previousPitch && pitch === 0) {
    pitch = previousPitch * 2;
  }

  return findPitchLabel(pitch);
}

interface IPitchDisplayProps {
  overtoneBuckets: OvertoneBucket[];
}

export default function PitchDisplay({ overtoneBuckets }: IPitchDisplayProps) {
  const pitch = useMemo(() => getPitch(overtoneBuckets), [overtoneBuckets]);
  const diffToTarget = pitch.targetValue - pitch.value;
  const prefix = diffToTarget < 0 ? "-" : "+";

  return (
    <div className="grid h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background">
      <div>{pitch.label}</div>
      <div className="font-mono">
        {prefix}
        {Math.abs(diffToTarget).toPrecision(4)}
      </div>
    </div>
  );
}
