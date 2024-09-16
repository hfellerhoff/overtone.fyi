import { useMemo } from "react";
import type { OvertoneBucket } from ".";
import { PitchAndLabel, pitches } from "./pitches";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

const PITCH_SENSITIVITY = 0.7;

function findPitchLabel(value: number) {
  let lowerValue: PitchAndLabel = [0, ""];
  let higherValue: PitchAndLabel = [0, ""];

  let i = 0;

  let entry = pitches[i];

  while (entry[0] < value) {
    lowerValue = entry;

    if (!pitches?.[i + 1]) {
      break;
    }

    i += 1;
    entry = pitches?.[i];
  }

  higherValue = pitches?.[i + 1] ?? pitches[i];

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

  // If can't find by overtones, just use the loudest pitch
  let pitchWeight = 0;
  if (!!previousPitch && pitch === 0) {
    overtones.forEach((overtone) => {
      if (overtone.weight > pitchWeight) {
        pitch = overtone.hz;
        pitchWeight = overtone.weight;
      }
    });
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

  const roundedPitch = Math.round(pitch.value * 100) / 100;

  return (
    <AnalysisSquareWrapper label="Pitch">
      <>
        <div className="font-mono">{pitch.label || "-"}</div>
        <div className="font-mono text-sm text-neutral-300">
          {roundedPitch}hz
        </div>
        <div className="font-mono text-sm text-neutral-500">
          {prefix}
          {Math.abs(diffToTarget).toPrecision(3)}hz
        </div>
      </>
    </AnalysisSquareWrapper>
  );
}
