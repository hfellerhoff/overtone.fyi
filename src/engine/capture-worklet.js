// AudioWorklet processor: down-mixes to mono and batches samples before
// posting them to the main thread (transferring the buffer, no copy).
// Kept as plain JS with no imports so it can be loaded straight from its URL.

const BATCH_FRAMES = 1024;

class CaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buffer = new Float32Array(BATCH_FRAMES);
    this.filled = 0;
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) return true;
    const channels = input.length;
    const frames = input[0].length;
    const scale = 1 / channels;
    for (let i = 0; i < frames; i++) {
      let s = 0;
      for (let c = 0; c < channels; c++) s += input[c][i];
      this.buffer[this.filled++] = s * scale;
      if (this.filled === BATCH_FRAMES) {
        this.port.postMessage(this.buffer, [this.buffer.buffer]);
        this.buffer = new Float32Array(BATCH_FRAMES);
        this.filled = 0;
      }
    }
    return true;
  }
}

registerProcessor("overtone-capture", CaptureProcessor);
