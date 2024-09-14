import { PropsWithChildren } from "react";

interface IAnalysisSquareWrapperProps {
  label: string;
}

export default function AnalysisSquareWrapper({
  label,
  children,
}: PropsWithChildren<IAnalysisSquareWrapperProps>) {
  return (
    <div className="flex flex-col h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background">
      <div className="pt-3 text-xs text-center text-neutral-500">{label}</div>
      <div className="flex flex-col items-center justify-center flex-1 px-1 pt-1 pb-3">
        {children}
      </div>
    </div>
  );
}
