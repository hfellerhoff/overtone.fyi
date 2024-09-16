import { useAtom, useAtomValue } from "jotai";
import { MicIcon, MicOffIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import AnalysisInfo from "./components/AnalysisInfo";
import FrequencyMarkers from "./components/FrequencyMarkers";
import {
  analyzerAtom,
  fftSizeAtom,
  frequencyLabelCanvasHeightAtom,
  frequencyLabelCanvasWidthAtom,
  isRecordingAtom,
  liveCanvasHeightAtom,
  liveCanvasWidthAtom,
  sampleRateAtom,
  TIMESERIES_CANVAS_WIDTHS,
  timeseriesCanvasHeightAtom,
  timeseriesCanvasWidthAtom,
} from "./lib/fft";
import { useUpdateAudioValues } from "./lib/useUpdateAudioValues";
import { useUpdateLiveCanvas } from "./lib/useUpdateLiveCanvas";
import { useUpdateTimeseriesCanvas } from "./lib/useUpdateTimeseriesCanvas";
import { useMediaQuery } from "./lib/utils";
import { useUpdateFrequencyLabelCanvas } from "./lib/useUpdateFrequencyLabelCanvas";

const TOP_BAR_HEIGHT = 128;
const VERTICAL_PADDING = 20;
const CANVAS_HEIGHT = `calc(var(--adjusted-height) - ${
  TOP_BAR_HEIGHT + VERTICAL_PADDING
}px)`;

export default function FFTCanvas() {
  const [analyzer, setAnalyzer] = useAtom(analyzerAtom);
  const [isRecording, setIsRecording] = useAtom(isRecordingAtom);

  const [sampleRate] = useAtom(sampleRateAtom);
  const [fftSize, setFFTSize] = useAtom(fftSizeAtom);

  const timeSeriesCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const [timeseriesCanvasWidth, setTimeseriesCanvasWidth] = useAtom(
    timeseriesCanvasWidthAtom
  );
  const [timeseriesCanvasHeight] = useAtom(timeseriesCanvasHeightAtom);
  const { registerTimeseriesCanvas, updateTimeseriesCanvas } =
    useUpdateTimeseriesCanvas();

  const liveCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const [liveCanvasWidth] = useAtom(liveCanvasWidthAtom);
  const [liveCanvasHeight] = useAtom(liveCanvasHeightAtom);
  const updateLiveCanvas = useUpdateLiveCanvas();

  const frequencyLabelCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const frequencyLabelCanvasWidth = useAtomValue(frequencyLabelCanvasWidthAtom);
  const frequencyLabelCanvasHeight = useAtomValue(
    frequencyLabelCanvasHeightAtom
  );
  const { registerFrequencyLabelCanvas, updateFrequencyLabelCanvas } =
    useUpdateFrequencyLabelCanvas();

  const updateAudioValues = useUpdateAudioValues();

  useEffect(() => {
    const handleKeypress = (ev: KeyboardEvent) => {
      if (ev.key === " ") {
        setIsRecording((prevIsRecording) => !prevIsRecording);
      }
    };

    window.addEventListener("keydown", handleKeypress);

    return () => {
      window.removeEventListener("keydown", handleKeypress);
    };
  });

  useEffect(() => {
    const getDevices = async () => {
      const userMediaStream = await navigator.mediaDevices.getUserMedia({
        audio: true,
      });
      const audioContext = new AudioContext({
        sampleRate,
      });
      const source = audioContext.createMediaStreamSource(userMediaStream);

      const analyser = audioContext.createAnalyser();

      source.connect(analyser);

      analyser.fftSize = fftSize;

      setAnalyzer(analyser);
    };

    getDevices();
  }, [
    fftSize,
    sampleRate,
    setAnalyzer,
    updateAudioValues,
    updateLiveCanvas,
    updateTimeseriesCanvas,
  ]);

  useEffect(() => {
    if (!analyzer || !isRecording) return;

    let id = 0;

    const update = (time: DOMHighResTimeStamp) => {
      const hzData = updateAudioValues(analyzer);

      if (timeSeriesCanvasRef.current) {
        updateTimeseriesCanvas(time, hzData);
      }
      if (liveCanvasRef.current) {
        updateLiveCanvas(liveCanvasRef.current, hzData);
      }
      if (frequencyLabelCanvasRef.current) {
        updateFrequencyLabelCanvas(time, hzData);
      }

      id = window.requestAnimationFrame(update);
    };

    registerTimeseriesCanvas(timeSeriesCanvasRef.current);
    registerFrequencyLabelCanvas(frequencyLabelCanvasRef.current);

    update(0);

    return () => {
      window.cancelAnimationFrame(id);
    };
  }, [
    analyzer,
    isRecording,
    registerFrequencyLabelCanvas,
    registerTimeseriesCanvas,
    updateAudioValues,
    updateFrequencyLabelCanvas,
    updateLiveCanvas,
    updateTimeseriesCanvas,
  ]);

  const isTablet = useMediaQuery("(max-width: 800px)");
  const isMobile = useMediaQuery("(max-width: 600px)");

  const [hasMounted, setHasMounted] = useState(false);
  useEffect(() => {
    if (!hasMounted) {
      setHasMounted(true);
      return;
    }

    if (isMobile) {
      if (timeseriesCanvasWidth === TIMESERIES_CANVAS_WIDTHS.DESKTOP) {
        setTimeseriesCanvasWidth(TIMESERIES_CANVAS_WIDTHS.MOBILE);
      }
    }

    if (!isMobile) {
      if (timeseriesCanvasWidth === TIMESERIES_CANVAS_WIDTHS.MOBILE) {
        setTimeseriesCanvasWidth(TIMESERIES_CANVAS_WIDTHS.DESKTOP);
      }
    }
  }, [
    hasMounted,
    isMobile,
    setFFTSize,
    setTimeseriesCanvasWidth,
    timeseriesCanvasWidth,
  ]);

  let liveCanvasWidthPx = liveCanvasWidth * 2;
  if (isMobile) {
    liveCanvasWidthPx = liveCanvasWidth;
  } else if (isTablet) {
    liveCanvasWidthPx = liveCanvasWidth * 1.5;
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
          onClick={() => setIsRecording((prevIsRecording) => !prevIsRecording)}
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
            width={liveCanvasWidth}
            height={liveCanvasHeight}
          />
        </div>
        <div
          className="relative h-full overflow-hidden"
          style={{
            width: frequencyLabelCanvasWidth,
          }}
        >
          <canvas
            ref={frequencyLabelCanvasRef}
            className="w-full h-full bg-black border rounded-lg border-input"
            width={frequencyLabelCanvasWidth}
            height={frequencyLabelCanvasHeight}
          />
        </div>
        <div className="relative w-full h-full overflow-hidden">
          <FrequencyMarkers
            canvas={timeSeriesCanvasRef.current}
            canvasHeight={timeseriesCanvasHeight}
          />
          <canvas
            ref={timeSeriesCanvasRef}
            className="w-full h-full bg-black border rounded-lg border-input"
            width={timeseriesCanvasWidth}
            height={timeseriesCanvasHeight}
          />
        </div>
      </main>
    </div>
  );
}
