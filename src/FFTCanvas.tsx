import { useAtom, useAtomValue } from "jotai";
import { MicIcon, MicOffIcon, RadioIcon } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import AnalysisInfo from "./components/AnalysisInfo";
import FrequencyMarkers from "./components/FrequencyMarkers";
import { toggleFullscreen } from "./engine/desktopWindow";
import { useAnalysis } from "./engine/useAnalysis";
import { panRange, zoomRange } from "./engine/viewport";
import {
  canvasHeightAtom,
  isRecordingAtom,
  TIMELINE_PIXELS_PER_SECOND,
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

  const { renderer, info, error, setViewport, isLive, historyEndRef } =
    useAnalysis();
  const infoRef = useRef(info);
  infoRef.current = info;

  /**
   * Wheel on the spectrogram:
   *  - plain vertical wheel / pinch (ctrl+wheel): zoom the frequency range
   *    around the cursor,
   *  - shift+wheel or horizontal wheel: scroll through time,
   *  - alt+wheel: pan the frequency range.
   */
  const handleWheel = useCallback(
    (ev: WheelEvent) => {
      const current = infoRef.current;
      if (!current) return;
      ev.preventDefault();
      const target = ev.currentTarget as HTMLElement;
      const rect = target.getBoundingClientRect();
      const nyquist = current.sampleRate / 2;
      const horizontal =
        ev.shiftKey || Math.abs(ev.deltaX) > Math.abs(ev.deltaY);
      const delta = horizontal
        ? ev.shiftKey && ev.deltaX === 0
          ? ev.deltaY
          : ev.deltaX
        : ev.deltaY;
      const pixels = ev.deltaMode === 1 ? delta * 16 : ev.deltaMode === 2 ? delta * rect.height : delta;

      if (horizontal) {
        // Positive deltaX (scroll right / swipe left) moves forward in time.
        const canvasPxPerScreenPx = timeseriesCanvasWidth / rect.width;
        const pxPerSecond = TIMELINE_PIXELS_PER_SECOND;
        const seconds = (pixels * canvasPxPerScreenPx) / pxPerSecond;
        setViewport((v) => {
          const end = v.viewEnd ?? historyEndRef.current;
          return { ...v, viewEnd: end + seconds };
        });
        return;
      }

      const anchor = 1 - (ev.clientY - rect.top) / rect.height;
      setViewport((v) => {
        const range = v.range ?? current.range;
        if (ev.altKey) {
          return { ...v, range: panRange(range, -pixels / rect.height, nyquist) };
        }
        const factor = Math.exp(-pixels * 0.002);
        return { ...v, range: zoomRange(range, anchor, factor, nyquist) };
      });
    },
    [setViewport, timeseriesCanvasWidth, historyEndRef],
  );


  const mainRef = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const el = mainRef.current;
    if (!el) return;
    el.addEventListener("wheel", handleWheel, { passive: false });
    return () => el.removeEventListener("wheel", handleWheel);
  }, [handleWheel]);

  const resetView = useCallback(() => {
    setViewport(() => ({ range: null, viewEnd: null }));
  }, [setViewport]);

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
      } else if (ev.key === "Escape" || ev.key === "0") {
        resetView();
      } else if (
        ev.key === "F11" ||
        ((ev.metaKey || ev.ctrlKey) && ev.shiftKey && ev.key.toLowerCase() === "f")
      ) {
        ev.preventDefault();
        void toggleFullscreen();
      }
    };
    window.addEventListener("keydown", handleKeypress);
    return () => window.removeEventListener("keydown", handleKeypress);
  }, [setIsRecording, resetView]);

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
        ref={mainRef}
        className="relative flex gap-2 pt-2 overflow-hidden rounded-lg"
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
          <FrequencyMarkers markers={info?.markers ?? []} />
          {!isLive && (
            <button
              className="absolute z-10 flex items-center gap-1 px-2 py-1 font-mono text-xs border rounded-md top-2 left-2 border-input bg-background/80 hover:bg-accent"
              onClick={resetView}
              title="Return to live (Esc)"
            >
              <RadioIcon size={12} /> Paused · back to live
            </button>
          )}
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
