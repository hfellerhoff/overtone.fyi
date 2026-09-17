import { useAtom, useAtomValue } from "jotai";
import { MicIcon, MicOffIcon } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import AnalysisInfo from "./components/AnalysisInfo";
import FrequencyMarkers from "./components/FrequencyMarkers";
import { useAnalysis } from "./engine/useAnalysis";
import {
  canvasHeightAtom,
  isRecordingAtom,
  TIMESERIES_CANVAS_WIDTHS,
  timeseriesCanvasWidthAtom,
} from "./lib/fft";
import { useMediaQuery } from "./lib/utils";

const TOP_BAR_HEIGHT = 128;
const VERTICAL_PADDING = 20;
const CANVAS_HEIGHT = `calc(var(--adjusted-height) - ${
  TOP_BAR_HEIGHT + VERTICAL_PADDING
}px)`;

const LIVE_CANVAS_WIDTH = 272;
const LABEL_CANVAS_WIDTH = 64;

export default function FFTCanvas() {
  const [isRecording, setIsRecording] = useAtom(isRecordingAtom);
  const canvasHeight = useAtomValue(canvasHeightAtom);
  const [timeseriesCanvasWidth, setTimeseriesCanvasWidth] = useAtom(
    timeseriesCanvasWidthAtom,
  );

  const timeSeriesCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const liveCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const frequencyLabelCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const { renderer, info, error } = useAnalysis();

  useLayoutEffect(() => {
    renderer.setCanvases(
      timeSeriesCanvasRef.current,
      liveCanvasRef.current,
      frequencyLabelCanvasRef.current,
    );
  }, [renderer, canvasHeight, timeseriesCanvasWidth]);

  useEffect(() => {
    const handleKeypress = (ev: KeyboardEvent) => {
      if (ev.key === " ") {
        setIsRecording((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handleKeypress);
    return () => window.removeEventListener("keydown", handleKeypress);
  }, [setIsRecording]);

  const isTablet = useMediaQuery("(max-width: 800px)");
  const isMobile = useMediaQuery("(max-width: 600px)");

  const [hasMounted, setHasMounted] = useState(false);
  useEffect(() => {
    if (!hasMounted) {
      setHasMounted(true);
      return;
    }
    if (isMobile && timeseriesCanvasWidth === TIMESERIES_CANVAS_WIDTHS.DESKTOP) {
      setTimeseriesCanvasWidth(TIMESERIES_CANVAS_WIDTHS.MOBILE);
    }
    if (!isMobile && timeseriesCanvasWidth === TIMESERIES_CANVAS_WIDTHS.MOBILE) {
      setTimeseriesCanvasWidth(TIMESERIES_CANVAS_WIDTHS.DESKTOP);
    }
  }, [hasMounted, isMobile, setTimeseriesCanvasWidth, timeseriesCanvasWidth]);

  let liveCanvasWidthPx = LIVE_CANVAS_WIDTH * 2;
  if (isMobile) {
    liveCanvasWidthPx = LIVE_CANVAS_WIDTH;
  } else if (isTablet) {
    liveCanvasWidthPx = LIVE_CANVAS_WIDTH * 1.5;
  }

  return (
    <div
      className="w-screen p-4 pt-0 bg-neutral-900"
      style={{
        height: "var(--adjusted-height)",
      }}
    >
      <div
        className="flex items-center gap-2 pt-4 overflow-x-auto overflow-y-hidden"
        style={{
          height: TOP_BAR_HEIGHT,
        }}
      >
        <button
          className="grid h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background hover:bg-accent hover:text-accent-foreground"
          onClick={() => setIsRecording((prev) => !prev)}
        >
          {isRecording ? (
            <span className="flex flex-col items-center gap-1">
              <MicOffIcon size={16} /> Stop
            </span>
          ) : (
            <span className="flex flex-col items-center gap-1">
              <MicIcon size={16} /> Start
            </span>
          )}
        </button>
        <AnalysisInfo />
      </div>
      {error && (
        <div className="px-2 py-1 mt-2 font-mono text-xs text-red-300 border border-red-900 rounded-md bg-red-950">
          {error}
        </div>
      )}
      <main
        className="flex gap-2 pt-2 overflow-hidden rounded-lg"
        style={{
          height: CANVAS_HEIGHT,
        }}
      >
        <div
          className="relative h-full overflow-hidden"
          style={{
            width: liveCanvasWidthPx,
          }}
        >
          <canvas
            ref={liveCanvasRef}
            className="w-full h-full bg-black border rounded-lg border-input"
            width={LIVE_CANVAS_WIDTH}
            height={canvasHeight}
          />
        </div>
        <div
          className="relative h-full overflow-hidden"
          style={{
            width: LABEL_CANVAS_WIDTH,
          }}
        >
          <canvas
            ref={frequencyLabelCanvasRef}
            className="w-full h-full bg-black border rounded-lg border-input"
            width={LABEL_CANVAS_WIDTH}
            height={canvasHeight}
          />
        </div>
        <div className="relative w-full h-full overflow-hidden">
          <FrequencyMarkers
            canvas={timeSeriesCanvasRef.current}
            canvasHeight={canvasHeight}
            markers={info?.markers ?? []}
          />
          <canvas
            ref={timeSeriesCanvasRef}
            className="w-full h-full bg-black border rounded-lg border-input"
            width={timeseriesCanvasWidth}
            height={canvasHeight}
          />
        </div>
      </main>
    </div>
  );
}
