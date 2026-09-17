import { useSyncExternalStore } from "react";
import { getLiveValues, subscribeLiveValues } from "@/engine/liveStore";
import AnalysisSquareWrapper from "./AnalysisSquareWrapper";

export default function PitchDisplay() {
  const pitch = useSyncExternalStore(subscribeLiveValues, getLiveValues);
  const diffToTarget = pitch.targetHz - pitch.pitchHz;
  const prefix = diffToTarget < 0 ? "-" : "+";

  const roundedPitch = Math.round(pitch.pitchHz * 100) / 100;

  return (
    <AnalysisSquareWrapper label="Pitch">
      <>
        <div className="font-mono">{pitch.note || "-"}</div>
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
