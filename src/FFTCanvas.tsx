import { useAtom, useAtomValue } from "jotai";
import { MicIcon, MicOffIcon, RadioIcon, Trash2Icon } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import AnalysisInfo from "./components/AnalysisInfo";
import FrequencyMarkers from "./components/FrequencyMarkers";
import { toggleFullscreen } from "./engine/desktopWindow";
import { useAnalysis } from "./engine/useAnalysis";
import { panRange, zoomRange } from "./engine/viewport";
import {
  canvasHeightAtom,
  isRecordingAtom,
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

  const {
    renderer,
    info,
    error,
    setViewport,
    isLive,
    historyEndRef,
    pxPerSecond,
    clear,
  } = useAnalysis();
  const infoRef = useRef(info);
  infoRef.current = info;

  // Size the timeline canvas to its container (in device pixels) so a
  // second of audio is always the same screen distance.
  const timelineContainerRef = useRef<HTMLDivElement | null>(null);
  useLayoutEffect(() => {
    const el = timelineContainerRef.current;
    if (!el) return;
    const measure = () => {
      const dpr = window.devicePixelRatio || 1;
      const next = Math.max(64, Math.round(el.clientWidth * dpr));
      setTimeseriesCanvasWidth((prev) => (prev === next ? prev : next));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [setTimeseriesCanvasWidth]);

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
    [setViewport, timeseriesCanvasWidth, historyEndRef, pxPerSecond],
  );

  /**
   * Click and drag on the spectrogram: vertical movement pans the frequency
   * range, horizontal movement scrolls through time. Dragging moves the
   * view, not the content: drag up to see higher frequencies, drag right to
   * move forward in time.
   */
  const drag = useRef<{
    id: number;
    x: number;
    y: number;
    moved: boolean;
  } | null>(null);
  const handlePointerDown = useCallback((ev: React.PointerEvent<HTMLElement>) => {
    if (ev.button !== 0) return;
    drag.current = { id: ev.pointerId, x: ev.clientX, y: ev.clientY, moved: false };
    ev.currentTarget.setPointerCapture(ev.pointerId);
  }, []);
  const handlePointerMove = useCallback(
    (ev: React.PointerEvent<HTMLElement>) => {
      const d = drag.current;
      const current = infoRef.current;
      if (!d || d.id !== ev.pointerId || !current) return;
      const dx = ev.clientX - d.x;
      const dy = ev.clientY - d.y;
      if (!d.moved && Math.abs(dx) < 3 && Math.abs(dy) < 3) return;
      d.moved = true;
      d.x = ev.clientX;
      d.y = ev.clientY;
      const rect = ev.currentTarget.getBoundingClientRect();
      const nyquist = current.sampleRate / 2;
      const canvasPxPerScreenPx = timeseriesCanvasWidth / rect.width;
      const seconds = (dx * canvasPxPerScreenPx) / pxPerSecond;
      setViewport((v) => {
        const range = v.range ?? current.range;
        const next = { ...v };
        if (dy !== 0) {
          // dragging up shows higher frequencies
          next.range = panRange(range, -dy / rect.height, nyquist);
        }
        if (dx !== 0) {
          // dragging right moves forward in time
          const end = v.viewEnd ?? historyEndRef.current;
          next.viewEnd = end + seconds;
        }
        return next;
      });
    },
    [setViewport, timeseriesCanvasWidth, historyEndRef, pxPerSecond],
  );
  const handlePointerUp = useCallback((ev: React.PointerEvent<HTMLElement>) => {
    if (drag.current?.id === ev.pointerId) {
      drag.current = null;
      ev.currentTarget.releasePointerCapture(ev.pointerId);
    }
  }, []);


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
        <button
          className="grid h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background hover:bg-accent hover:text-accent-foreground"
          onClick={() => void clear()}
          title="Clear the recording"
        >
          <span className="flex flex-col items-center gap-1">
            <Trash2Icon size={16} /> Clear
          </span>
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
        className="relative flex gap-2 pt-2 overflow-hidden rounded-lg select-none touch-none cursor-grab active:cursor-grabbing"
        style={{
          height: CANVAS_HEIGHT,
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
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
        <div
          ref={timelineContainerRef}
          className="relative w-full h-full overflow-hidden"
        >
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
