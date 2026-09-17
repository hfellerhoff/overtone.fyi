/**
 * Frame packet produced by the Rust engine (see crates/overtone-core/src/packet.rs).
 * All values little-endian.
 */
export const PACKET_MAGIC = 0x4e54564f;
export const PACKET_VERSION = 1;
export const HEADER_LEN = 40;
export const FLAG_NEW_AUDIO = 1;

export interface FramePacket {
  seq: number;
  height: number;
  flags: number;
  pitchHz: number;
  targetHz: number;
  /** Index into EngineInfo.notes, or -1. */
  note: number;
  sampleRate: number;
  /** RGBA column, index 0 = top row (height * 4 bytes). */
  column: Uint8ClampedArray;
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
  const columnOffset = bytes.byteOffset + HEADER_LEN;
  const liveOffset = columnOffset + height * 4;
  // The live floats are 4-byte aligned only when the packet start is; copy
  // when it is not (wasm views are aligned, IPC ArrayBuffers start at 0).
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
    column: new Uint8ClampedArray(bytes.buffer, columnOffset, height * 4),
    live,
  };
}
