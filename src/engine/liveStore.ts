/**
 * Tiny external store for per-frame values the UI shows as text. Components
 * subscribe with `useSyncExternalStore`; the render loop writes to it and
 * only notifies when the displayed values actually change, so React never
 * re-renders at frame rate.
 */
export interface LiveValues {
  pitchHz: number;
  targetHz: number;
  note: string;
}

const EMPTY: LiveValues = { pitchHz: 0, targetHz: 0, note: "" };

let current: LiveValues = EMPTY;
const listeners = new Set<() => void>();

export function getLiveValues(): LiveValues {
  return current;
}

export function subscribeLiveValues(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setLiveValues(next: LiveValues) {
  if (
    next.pitchHz === current.pitchHz &&
    next.targetHz === current.targetHz &&
    next.note === current.note
  ) {
    return;
  }
  current = next;
  listeners.forEach((l) => l());
}

export function resetLiveValues() {
  setLiveValues(EMPTY);
}
