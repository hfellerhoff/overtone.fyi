import {
  PopoverTrigger,
  PopoverContent,
  Popover,
} from "@/components/ui/popover";
import { PropsWithChildren } from "react";
import { ExternalLinkIcon, HelpCircleIcon } from "lucide-react";

interface IAnalysisSquareWrapperProps {
  label: string;
  tooltip?: string;
  tooltipLink?: string;
}

export default function AnalysisSquareWrapper({
  label,
  children,
  tooltip,
  tooltipLink,
}: PropsWithChildren<IAnalysisSquareWrapperProps>) {
  if (!tooltip) {
    return (
      <div className="flex flex-col h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background">
        <div className="flex items-center gap-1 pt-3 text-xs text-center text-neutral-500">
          {label}
        </div>
        <div className="flex flex-col items-center justify-center flex-1 px-1 pt-1 pb-3">
          {children}
        </div>
      </div>
    );
  }

  return (
    <Popover>
      <div className="flex flex-col h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background">
        <div className="flex items-center gap-1 pt-3 text-xs text-center text-neutral-500">
          {label}
          <PopoverTrigger>
            <HelpCircleIcon size={12} />
          </PopoverTrigger>
        </div>
        <div className="flex flex-col items-center justify-center flex-1 px-1 pt-1 pb-3">
          {children}
        </div>
      </div>
      <PopoverContent className="text-sm">
        <p>{tooltip}</p>
        <p>
          {tooltipLink && (
            <a
              href={tooltipLink}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1 mt-2 text-neutral-500 hover:underline"
            >
              Read More <ExternalLinkIcon size={12} />
            </a>
          )}
        </p>
      </PopoverContent>
    </Popover>
  );
}
