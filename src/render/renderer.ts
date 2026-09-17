import { parsePacket } from "@/engine/packet";
import type { EngineInfo } from "@/engine/types";

/**
 * Draws frame packets onto the three canvases. All analysis has already been
 * done in Rust; this only blits pixels.
 */
export class SpectrogramRenderer {
  private timeseries: CanvasRenderingContext2D | null = null;
  private live: CanvasRenderingContext2D | null = null;
  private label: CanvasRenderingContext2D | null = null;
  private liveGradient: CanvasGradient | null = null;
  private column: ImageData | null = null;
  private columnCanvas: HTMLCanvasElement | null = null;
  private lastSeq = -1;
  /** Timeline scroll speed in canvas pixels per second. */
  private pixelsPerSecond = 240;
  private lastTimeMs: number | null = null;
  private scrollAccumulator = 0;

  setTimelineSpeed(pixelsPerSecond: number) {
    this.pixelsPerSecond = pixelsPerSecond;
  }

  setCanvases(
    timeseries: HTMLCanvasElement | null,
    live: HTMLCanvasElement | null,
    label: HTMLCanvasElement | null,
  ) {
    this.timeseries = timeseries?.getContext("2d", { alpha: false }) ?? null;
    this.live = live?.getContext("2d", { alpha: false }) ?? null;
    this.label = label?.getContext("2d", { alpha: false }) ?? null;
    this.liveGradient = null;
    this.column = null;
    this.columnCanvas = null;
    this.lastTimeMs = null;
    this.scrollAccumulator = 0;
    if (this.timeseries) {
      this.timeseries.imageSmoothingEnabled = false;
      this.timeseries.fillStyle = "black";
      this.timeseries.fillRect(0, 0, this.timeseries.canvas.width, this.timeseries.canvas.height);
    }
  }

  drawLabelStrip(strip: Uint8Array, info: EngineInfo) {
    const ctx = this.label;
    if (!ctx) return;
    // Copy into a fresh ArrayBuffer-backed array (wasm memory views and IPC
    // buffers may be SharedArrayBuffer-typed as far as TypeScript knows).
    const pixels = new Uint8ClampedArray(strip.byteLength);
    pixels.set(strip);
    const image = new ImageData(pixels, info.labelWidth, info.height);
    ctx.putImageData(image, 0, 0);
  }

  clearHistory() {
    const ctx = this.timeseries;
    if (!ctx) return;
    ctx.fillStyle = "black";
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    this.lastSeq = -1;
    this.lastTimeMs = null;
    this.scrollAccumulator = 0;
  }

  /**
   * Draw a frame packet. `timeMs` drives the timeline scroll so the
   * horizontal axis stays linear in time regardless of frame rate.
   * Returns the parsed packet so the caller can update text displays.
   */
  draw(bytes: Uint8Array, timeMs: number) {
    const packet = parsePacket(bytes);
    if (packet.seq === this.lastSeq) return packet;
    this.lastSeq = packet.seq;
    this.drawTimeseries(packet.column, packet.height, timeMs);
    this.drawLive(packet.live, packet.height);
    return packet;
  }

  private drawTimeseries(column: Uint8ClampedArray, height: number, timeMs: number) {
    const ctx = this.timeseries;
    if (!ctx) return;
    const { width, height: canvasHeight } = ctx.canvas;
    if (canvasHeight !== height) return;

    // How many pixels the timeline advances for the elapsed time.
    let advance = 1;
    if (this.lastTimeMs !== null) {
      this.scrollAccumulator +=
        ((timeMs - this.lastTimeMs) / 1000) * this.pixelsPerSecond;
      advance = Math.floor(this.scrollAccumulator);
      this.scrollAccumulator -= advance;
    }
    this.lastTimeMs = timeMs;
    if (advance <= 0) return;
    advance = Math.min(advance, width);

    // Scroll the existing image to the right (GPU blit), then paint the new
    // column stretched over the pixels that were vacated.
    ctx.drawImage(ctx.canvas, advance, 0);
    if (!this.column || this.column.height !== height) {
      this.column = new ImageData(1, height);
      this.columnCanvas = document.createElement("canvas");
      this.columnCanvas.width = 1;
      this.columnCanvas.height = height;
    }
    this.column.data.set(column);
    if (advance === 1 || !this.columnCanvas) {
      ctx.putImageData(this.column, 0, 0);
      return;
    }
    const columnCtx = this.columnCanvas.getContext("2d");
    if (!columnCtx) return;
    columnCtx.putImageData(this.column, 0, 0);
    ctx.drawImage(this.columnCanvas, 0, 0, 1, height, 0, 0, advance, height);
  }

  private drawLive(live: Float32Array, height: number) {
    const ctx = this.live;
    if (!ctx) return;
    const { width, height: canvasHeight } = ctx.canvas;
    if (canvasHeight !== height) return;
    ctx.fillStyle = "black";
    ctx.fillRect(0, 0, width, height);
    if (!this.liveGradient) {
      const gradient = ctx.createLinearGradient(0, 0, 280, 0);
      gradient.addColorStop(0, "red");
      gradient.addColorStop(0.25, "orange");
      gradient.addColorStop(0.5, "yellow");
      gradient.addColorStop(0.75, "cyan");
      gradient.addColorStop(1, "darkblue");
      this.liveGradient = gradient;
    }
    ctx.fillStyle = this.liveGradient;
    for (let y = 0; y < height; y++) {
      const value = live[y];
      if (value > 0) {
        ctx.fillRect(width - value, y, value, 1);
      }
    }
  }
}
