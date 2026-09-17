import type { FreqRange } from "./types";

/**
 * The user's view of the recording: which frequency range is on screen and
 * where the timeline sits in time. Kept outside React state because it
 * changes on every wheel event and is read by the frame loop.
 */
export interface Viewport {
  /** Visible frequency range, or null for the scale's preset. */
  range: FreqRange | null;
  /** Newest edge of the timeline in seconds, or null to follow live. */
  viewEnd: number | null;
}

/** Hard limits, mirrored from crates/overtone-core/src/mapping.rs. */
export const ABSOLUTE_MIN_HZ = 60;
export const ABSOLUTE_MAX_HZ = 20000;
export const MIN_SPAN_RATIO = 1.26;

export function clampRange(range: FreqRange, nyquistIn: number): FreqRange {
  const nyquist = Math.min(nyquistIn, ABSOLUTE_MAX_HZ);
  let minHz = Math.max(ABSOLUTE_MIN_HZ, range.minHz);
  let maxHz = Math.min(nyquist, range.maxHz);
  if (maxHz / minHz < MIN_SPAN_RATIO) {
    const centre = Math.sqrt(minHz * maxHz);
    minHz = centre / Math.sqrt(MIN_SPAN_RATIO);
    maxHz = centre * Math.sqrt(MIN_SPAN_RATIO);
    if (minHz < ABSOLUTE_MIN_HZ) {
      minHz = ABSOLUTE_MIN_HZ;
      maxHz = minHz * MIN_SPAN_RATIO;
    }
    if (maxHz > nyquist) {
      maxHz = nyquist;
      minHz = maxHz / MIN_SPAN_RATIO;
    }
  }
  return { minHz, maxHz };
}

/**
 * Zoom a log-frequency range by `factor` (> 1 zooms in) keeping the
 * frequency at `anchor` (0 = bottom, 1 = top) fixed on screen.
 */
export function zoomRange(
  range: FreqRange,
  anchor: number,
  factor: number,
  nyquist: number,
): FreqRange {
  // zooming out past the bounds just stops at them
  const logMin = Math.log(range.minHz);
  const logMax = Math.log(range.maxHz);
  const span = logMax - logMin;
  const pivot = logMin + span * anchor;
  const newSpan = span / factor;
  return clampRange(
    {
      minHz: Math.exp(pivot - newSpan * anchor),
      maxHz: Math.exp(pivot + newSpan * (1 - anchor)),
    },
    nyquist,
  );
}

/** Pan a log-frequency range by a fraction of its height (positive = up). */
export function panRange(range: FreqRange, fraction: number, nyquistIn: number): FreqRange {
  const nyquist = Math.min(nyquistIn, ABSOLUTE_MAX_HZ);
  const span = Math.log(range.maxHz / range.minHz);
  let shift = span * fraction;
  // do not pan past the limits; keep the span intact
  const minShift = Math.log(ABSOLUTE_MIN_HZ / range.minHz);
  const maxShift = Math.log(nyquist / range.maxHz);
  shift = Math.min(Math.max(shift, minShift), maxShift);
  return {
    minHz: range.minHz * Math.exp(shift),
    maxHz: range.maxHz * Math.exp(shift),
  };
}
