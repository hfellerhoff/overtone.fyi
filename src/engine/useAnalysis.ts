import { useAtomValue } from "jotai";
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  analyzerScaleAtom,
  canvasHeightAtom,
  coloringMethodAtom,
  fftSizeAtom,
  frequencyLabelMethodAtom,
  isRecordingAtom,
  TIMELINE_PIXELS_PER_SECOND,
  timeseriesCanvasWidthAtom,
} from "@/lib/fft";
import { SpectrogramRenderer } from "@/render/renderer";
import { createBackend, type AnalysisBackend } from "./backend";
import { resetLiveValues, setLiveValues } from "./liveStore";
import { FLAG_LIVE } from "./packet";
import type { DisplayConfig, EngineInfo, FreqRange } from "./types";
import type { Viewport } from "./viewport";

/** Minimum interval between engine reconfigurations while zooming. */
const RANGE_THROTTLE_MS = 60;

export interface AnalysisHandle {
  renderer: SpectrogramRenderer;
  info: EngineInfo | null;
  backendKind: AnalysisBackend["kind"] | null;
  error: string | null;
  /** Current view; mutate through `setViewport`. */
  viewport: Viewport;
  setViewport(update: (v: Viewport) => Viewport): void;
  /** Whether the timeline is following the newest audio. */
  isLive: boolean;
  /** Newest recorded time in seconds, updated every frame. */
  historyEndRef: React.MutableRefObject<number>;
}

/**
 * Owns the backend lifecycle: creates it, pushes configuration changes,
 * starts/stops capture and runs the frame loop.
 */
export function useAnalysis(): AnalysisHandle {
  const rendererRef = useRef<SpectrogramRenderer>();
  if (!rendererRef.current) {
    rendererRef.current = new SpectrogramRenderer();
  }
  const renderer = rendererRef.current;

  const [backend, setBackend] = useState<AnalysisBackend | null>(null);
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLive, setIsLive] = useState(true);
  const historyEndRef = useRef(0);

  const isRecording = useAtomValue(isRecordingAtom);
  const fftSize = useAtomValue(fftSizeAtom);
  const height = useAtomValue(canvasHeightAtom);
  const width = useAtomValue(timeseriesCanvasWidthAtom);
  const scale = useAtomValue(analyzerScaleAtom);
  const coloring = useAtomValue(coloringMethodAtom);
  const labeling = useAtomValue(frequencyLabelMethodAtom);

  // The viewport changes on every wheel event; keep it in a ref that the
  // frame loop reads, and mirror the frequency range into state so the
  // engine gets reconfigured (and the labels and history redrawn). Range
  // updates are throttled so a fast zoom gesture redraws steadily instead
  // of once per wheel event.
  const viewportRef = useRef<Viewport>({ range: null, viewEnd: null });
  const [range, setRange] = useState<FreqRange | null>(null);
  const rangeTimer = useRef<number | null>(null);
  const applyRange = useCallback(() => {
    rangeTimer.current = null;
    const next = viewportRef.current.range;
    setRange((prev) =>
      prev?.minHz === next?.minHz && prev?.maxHz === next?.maxHz ? prev : next,
    );
  }, []);
  const setViewport = useCallback(
    (update: (v: Viewport) => Viewport) => {
      viewportRef.current = update(viewportRef.current);
      if (rangeTimer.current === null) {
        // leading edge applies at once; later changes coalesce into one
        // trailing update
        applyRange();
        rangeTimer.current = window.setTimeout(applyRange, RANGE_THROTTLE_MS);
      }
    },
    [applyRange],
  );
  useEffect(
    () => () => {
      if (rangeTimer.current !== null) window.clearTimeout(rangeTimer.current);
    },
    [],
  );

  // Changing scale resets any zoom.
  const previousScale = useRef(scale);
  useEffect(() => {
    if (previousScale.current !== scale) {
      previousScale.current = scale;
      setViewport((v) => ({ ...v, range: null }));
    }
  }, [scale, setViewport]);

  useEffect(() => {
    let cancelled = false;
    let created: AnalysisBackend | null = null;
    createBackend()
      .then((b) => {
        if (cancelled) {
          b.dispose();
          return;
        }
        created = b;
        setBackend(b);
      })
      .catch((e) => setError(String(e)));
    return () => {
      cancelled = true;
      created?.dispose();
    };
  }, []);

  // Configuration changes: reconfigure the engine and redraw the label strip.
  const infoRef = useRef<EngineInfo | null>(null);
  const configRef = useRef<DisplayConfig | null>(null);
  useEffect(() => {
    if (!backend) return;
    let cancelled = false;
    const config: DisplayConfig = {
      fftSize,
      height,
      scale,
      coloring,
      labeling,
      range,
      bands: "single",
    };
    configRef.current = config;
    (async () => {
      try {
        const next = await backend.configure(config);
        const strip = await backend.labelStrip();
        if (cancelled) return;
        infoRef.current = next;
        setInfo(next);
        renderer.drawLabelStrip(strip, next);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend, fftSize, height, scale, coloring, labeling, range, renderer]);

  // Capture + frame loop.
  useEffect(() => {
    if (!backend || !isRecording) return;
    let cancelled = false;
    let raf = 0;

    const loop = async () => {
      if (cancelled) return;
      try {
        const bytes = await backend.frame({
          width,
          pxPerSecond: TIMELINE_PIXELS_PER_SECOND,
          viewEnd: viewportRef.current.viewEnd,
        });
        if (cancelled) return;
        if (bytes) {
          const packet = renderer.draw(bytes);
          const notes = infoRef.current?.notes;
          setLiveValues({
            pitchHz: packet.pitchHz,
            targetHz: packet.targetHz,
            note: packet.note >= 0 && notes ? notes[packet.note] : "",
          });
          historyEndRef.current = packet.historyEnd;
          const live = (packet.flags & FLAG_LIVE) !== 0;
          if (live && viewportRef.current.viewEnd !== null) {
            // scrolled back to the newest edge: follow live again
            viewportRef.current = { ...viewportRef.current, viewEnd: null };
          }
          setIsLive(live);
        }
      } catch (e) {
        setError(String(e));
        return;
      }
      raf = requestAnimationFrame(loop);
    };

    backend
      .start()
      .then((status) => {
        if (cancelled) return;
        if (status.sampleRate !== infoRef.current?.sampleRate && configRef.current) {
          // The device's real sample rate can differ from the default.
          backend.configure(configRef.current).then((next) => {
            if (cancelled) return;
            infoRef.current = next;
            setInfo(next);
          });
        }
        raf = requestAnimationFrame(loop);
      })
      .catch((e) => setError(String(e)));

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      backend.stop().catch(() => undefined);
      resetLiveValues();
    };
  }, [backend, isRecording, renderer, width]);

  return {
    renderer,
    info,
    backendKind: backend?.kind ?? null,
    error,
    viewport: viewportRef.current,
    setViewport,
    isLive,
    historyEndRef,
  };
}
