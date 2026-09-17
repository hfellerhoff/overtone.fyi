import type { BandPreset } from "./types";

/** A frequency band and its window length as a divisor of the base FFT size. */
export interface Band {
  maxHz: number | null;
  divisor: number;
}

/**
 * Relative window layouts; mirrors `BandPreset::bands` in
 * crates/overtone-core/src/spectrum.rs. Every band is a fixed fraction of the
 * base window, so the Resolution setting scales them all together.
 */
export function bandsForPreset(preset: BandPreset): Band[] {
  switch (preset) {
    case "single":
      return [{ maxHz: null, divisor: 1 }];
    case "balanced":
      return [
        { maxHz: 250, divisor: 1 },
        { maxHz: 500, divisor: 2 },
        { maxHz: 1000, divisor: 4 },
        { maxHz: null, divisor: 8 },
      ];
    case "sharp":
      return [
        { maxHz: 125, divisor: 1 },
        { maxHz: 250, divisor: 2 },
        { maxHz: 500, divisor: 4 },
        { maxHz: 1000, divisor: 8 },
        { maxHz: 2000, divisor: 16 },
        { maxHz: null, divisor: 32 },
      ];
  }
}

export function formatWindow(fftSize: number): string {
  return fftSize >= 1024 ? `${fftSize / 1024}k` : `${fftSize}`;
}

export function formatHz(hz: number): string {
  return hz >= 1000 ? `${hz / 1000}k` : `${hz}`;
}
