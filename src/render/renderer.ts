import { FLAG_NEW_AUDIO, parsePacket } from "@/engine/packet";
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
  private lastSeq = -1;

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
  }

  /** Returns the parsed packet so the caller can update text displays. */
  draw(bytes: Uint8Array) {
    const packet = parsePacket(bytes);
    if (packet.seq === this.lastSeq) return packet;
    this.lastSeq = packet.seq;
    // A frame with no new audio would just repeat the previous column.
    if (packet.flags & FLAG_NEW_AUDIO) {
      this.drawTimeseries(packet.column, packet.height);
    }
    this.drawLive(packet.live, packet.height);
    return packet;
  }

  private drawTimeseries(column: Uint8ClampedArray, height: number) {
    const ctx = this.timeseries;
    if (!ctx) return;
    const { width, height: canvasHeight } = ctx.canvas;
    if (canvasHeight !== height) return;
    // Scroll the existing image one pixel to the right (GPU blit), then
    // paint the new column at x = 0.
    ctx.drawImage(ctx.canvas, 1, 0);
    if (!this.column || this.column.height !== height) {
      this.column = new ImageData(1, height);
    }
    this.column.data.set(column);
    ctx.putImageData(this.column, 0, 0);
    void width;
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
