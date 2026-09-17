import { useAtomValue } from "jotai";
import { useEffect, useRef, useState } from "react";
import {
  analyzerScaleAtom,
  canvasHeightAtom,
  coloringMethodAtom,
  fftSizeAtom,
  frequencyLabelMethodAtom,
  isRecordingAtom,
} from "@/lib/fft";
import { SpectrogramRenderer } from "@/render/renderer";
import { createBackend, type AnalysisBackend } from "./backend";
import { resetLiveValues, setLiveValues } from "./liveStore";
import type { DisplayConfig, EngineInfo } from "./types";

export interface AnalysisHandle {
  renderer: SpectrogramRenderer;
  info: EngineInfo | null;
  backendKind: AnalysisBackend["kind"] | null;
  error: string | null;
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

  const isRecording = useAtomValue(isRecordingAtom);
  const fftSize = useAtomValue(fftSizeAtom);
  const height = useAtomValue(canvasHeightAtom);
  const scale = useAtomValue(analyzerScaleAtom);
  const coloring = useAtomValue(coloringMethodAtom);
  const labeling = useAtomValue(frequencyLabelMethodAtom);

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
  useEffect(() => {
    if (!backend) return;
    let cancelled = false;
    const config: DisplayConfig = { fftSize, height, scale, coloring, labeling };
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
  }, [backend, fftSize, height, scale, coloring, labeling, renderer]);

  // Capture + frame loop.
  useEffect(() => {
    if (!backend || !isRecording) return;
    let cancelled = false;
    let raf = 0;

    const loop = async () => {
      if (cancelled) return;
      try {
        const bytes = await backend.frame();
        if (cancelled) return;
        if (bytes) {
          const packet = renderer.draw(bytes);
          const notes = infoRef.current?.notes;
          setLiveValues({
            pitchHz: packet.pitchHz,
            targetHz: packet.targetHz,
            note: packet.note >= 0 && notes ? notes[packet.note] : "",
          });
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
        if (status.sampleRate !== infoRef.current?.sampleRate) {
          // The device's real sample rate can differ from the default.
          backend.configure({ fftSize, height, scale, coloring, labeling }).then((next) => {
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
    // Config values are read once on (re)start; changes go through `configure`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, isRecording, renderer]);

  return { renderer, info, backendKind: backend?.kind ?? null, error };
}
