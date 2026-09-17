import { FLAG_FULL, parsePacket, type FramePacket } from "@/engine/packet";
import type { EngineInfo } from "@/engine/types";

/**
 * Draws frame packets onto the three canvases. All analysis has already been
 * done in Rust; this only blits pixels. The timeline is kept as an image the
 * engine updates incrementally: it tells us how far to shift the existing
 * picture and sends only the columns that changed.
 */
export class SpectrogramRenderer {
  private timeseries: CanvasRenderingContext2D | null = null;
  private live: CanvasRenderingContext2D | null = null;
  private label: CanvasRenderingContext2D | null = null;
  private liveGradient: CanvasGradient | null = null;
  private scratch: ImageData | null = null;
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
    this.scratch = null;
    this.clearHistory();
  }

  drawLabelStrip(strip: Uint8Array, info: EngineInfo) {
    const ctx = this.label;
    if (!ctx) return;
    const pixels = new Uint8ClampedArray(strip.byteLength);
    pixels.set(strip);
    ctx.putImageData(new ImageData(pixels, info.labelWidth, info.height), 0, 0);
  }

  clearHistory() {
    const ctx = this.timeseries;
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "black";
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    this.lastSeq = -1;
  }

  /** Returns the parsed packet so the caller can update text displays. */
  draw(bytes: Uint8Array): FramePacket {
    const packet = parsePacket(bytes);
    if (packet.seq === this.lastSeq) return packet;
    this.lastSeq = packet.seq;
    this.drawTimeseries(packet);
    this.drawLive(packet.live, packet.height);
    return packet;
  }

  private drawTimeseries(packet: FramePacket) {
    const ctx = this.timeseries;
    if (!ctx) return;
    const { width, height } = ctx.canvas;
    if (height !== packet.height || width !== packet.width) return;
    const full = (packet.flags & FLAG_FULL) !== 0;
    if (!full && packet.shift !== 0) {
      ctx.drawImage(ctx.canvas, packet.shift, 0);
    }
    if (packet.columns === 0) return;
    if (
      !this.scratch ||
      this.scratch.width !== packet.columns ||
      this.scratch.height !== height
    ) {
      this.scratch = new ImageData(packet.columns, height);
    }
    this.scratch.data.set(packet.pixels);
    ctx.putImageData(this.scratch, packet.columnStart, 0);
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
