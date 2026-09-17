/**
 * Frame packet produced by the Rust engine (see crates/overtone-core/src/packet.rs).
 * All values little-endian.
 */
export const PACKET_MAGIC = 0x4e54564f;
export const PACKET_VERSION = 2;
export const HEADER_LEN = 68;
export const FLAG_NEW_AUDIO = 1;
export const FLAG_LIVE = 2;
export const FLAG_FULL = 4;

export interface FramePacket {
  seq: number;
  height: number;
  flags: number;
  pitchHz: number;
  targetHz: number;
  /** Index into EngineInfo.notes, or -1. */
  note: number;
  sampleRate: number;
  /** Pixels to move the existing timeline right (negative = left). */
  shift: number;
  /** x position of the first included column. */
  columnStart: number;
  /** Number of timeline columns included. */
  columns: number;
  /** Timeline width the engine rendered for. */
  width: number;
  /** Time at the newest edge of the view, seconds. */
  viewEnd: number;
  historyStart: number;
  historyEnd: number;
  pxPerSecond: number;
  /** RGBA, row-major, `columns × height`. */
  pixels: Uint8ClampedArray;
  /** Live bar length per row, index 0 = top row. */
  live: Float32Array;
}

export function parsePacket(bytes: Uint8Array): FramePacket {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0, true) !== PACKET_MAGIC) {
    throw new Error("bad frame packet magic");
  }
  const version = view.getUint32(4, true);
  if (version !== PACKET_VERSION) {
    throw new Error(`unsupported frame packet version ${version}`);
  }
  const height = view.getUint32(12, true);
  const columns = view.getUint32(40, true);
  const pixelsOffset = bytes.byteOffset + HEADER_LEN;
  const pixelsLen = columns * height * 4;
  const liveOffset = pixelsOffset + pixelsLen;
  // Float32Array needs 4-byte alignment; copy when the packet is not aligned.
  const live =
    liveOffset % 4 === 0
      ? new Float32Array(bytes.buffer, liveOffset, height)
      : new Float32Array(bytes.buffer.slice(liveOffset, liveOffset + height * 4));
  return {
    seq: view.getUint32(8, true),
    height,
    flags: view.getUint32(16, true),
    pitchHz: view.getFloat32(20, true),
    targetHz: view.getFloat32(24, true),
    note: view.getInt32(28, true),
    sampleRate: view.getFloat32(32, true),
    shift: view.getInt32(36, true),
    columnStart: view.getUint32(64, true),
    columns,
    width: view.getUint32(44, true),
    viewEnd: view.getFloat32(48, true),
    historyStart: view.getFloat32(52, true),
    historyEnd: view.getFloat32(56, true),
    pxPerSecond: view.getFloat32(60, true),
    pixels: new Uint8ClampedArray(bytes.buffer, pixelsOffset, pixelsLen),
    live,
  };
}
