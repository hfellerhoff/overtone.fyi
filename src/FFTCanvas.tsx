import { useAtom, useAtomValue } from "jotai";
import { useEffect, useRef } from "react";
import {
  analyzerAtom,
  fftSizeAtom,
  frequencyMarkersAtom,
  isRecordingAtom,
  liveCanvasHeightAtom,
  liveCanvasWidthAtom,
  sampleRateAtom,
  timeseriesCanvasHeightAtom,
  timeseriesCanvasWidthAtom,
} from "./lib/fft";
import { useUpdateAudioValues } from "./lib/useUpdateAudioValues";
import { useUpdateLiveCanvas } from "./lib/useUpdateLiveCanvas";
import { useUpdateTimeseriesCanvas } from "./lib/useUpdateTimeseriesCanvas";
import { Button } from "./components/ui/button";
import { MicIcon, MicOffIcon } from "lucide-react";

export default function FFTCanvas() {
  const timeSeriesCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const liveCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const [analyzer, setAnalyzer] = useAtom(analyzerAtom);
  const [isRecording, setIsRecording] = useAtom(isRecordingAtom);

  const [sampleRate] = useAtom(sampleRateAtom);
  const [fftSize] = useAtom(fftSizeAtom);
  const [timeseriesCanvasWidth] = useAtom(timeseriesCanvasWidthAtom);
  const [timeseriesCanvasHeight] = useAtom(timeseriesCanvasHeightAtom);
  const [liveCanvasWidth] = useAtom(liveCanvasWidthAtom);
  const [liveCanvasHeight] = useAtom(liveCanvasHeightAtom);
  const frequencyMarkers = useAtomValue(frequencyMarkersAtom);

  const updateAudioValues = useUpdateAudioValues();
  const updateTimeseriesCanvas = useUpdateTimeseriesCanvas();
  const updateLiveCanvas = useUpdateLiveCanvas();

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

    const interval = setInterval(() => {
      const hzData = updateAudioValues(analyzer);

      if (timeSeriesCanvasRef.current) {
        updateTimeseriesCanvas(timeSeriesCanvasRef.current, hzData);
      }

      if (liveCanvasRef.current) {
        updateLiveCanvas(liveCanvasRef.current, hzData);
      }
    }, 10);

    return () => {
      clearTimeout(interval);
    };
  }, [
    analyzer,
    isRecording,
    updateAudioValues,
    updateLiveCanvas,
    updateTimeseriesCanvas,
  ]);

  const canvasHeightFactor =
    (timeSeriesCanvasRef.current?.clientHeight ?? 0) / timeseriesCanvasHeight;

  return (
    <div className="w-screen h-screen p-4 pt-0 bg-neutral-900">
      <div className="flex items-center h-16 pt-4">
        <Button
          size="sm"
          variant="outline"
          onClick={() => setIsRecording((prevIsRecording) => !prevIsRecording)}
        >
          {isRecording ? (
            <span className="flex gap-1">
              <MicOffIcon size={16} /> Stop
            </span>
          ) : (
            <span className="flex gap-1">
              <MicIcon size={16} /> Start
            </span>
          )}
        </Button>
      </div>
      <main className="flex h-[calc(100vh-80px)] gap-1">
        <div
          className="relative h-full overflow-hidden"
          style={{
            width: liveCanvasWidth * 2,
          }}
        >
          {frequencyMarkers.map((marker) => (
            <div
              key={marker[0]}
              className="absolute right-0 flex items-center justify-center h-2.5 text-[10px] font-mono text-right select-none"
              style={{
                bottom: canvasHeightFactor * marker[1] - 5,
              }}
            >
              {marker[0]}hz
            </div>
          ))}
          <canvas
            ref={liveCanvasRef}
            className="w-full h-full bg-black rounded-lg"
            width={liveCanvasWidth}
            height={liveCanvasHeight}
          />
        </div>
        <div className="relative w-full h-full overflow-hidden">
          {frequencyMarkers.map((marker) => (
            <div
              key={marker[0]}
              className="absolute right-0 flex items-center justify-center h-2.5 text-[10px] font-mono text-right select-none"
              style={{
                bottom: canvasHeightFactor * marker[1] - 5,
              }}
            >
              {marker[0]}hz
            </div>
          ))}
          <canvas
            ref={timeSeriesCanvasRef}
            className="w-full h-full bg-black rounded-lg"
            width={timeseriesCanvasWidth}
            height={timeseriesCanvasHeight}
          />
        </div>
      </main>
    </div>
  );
}
