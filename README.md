# overtone.fyi

Real-time overtone / spectrogram analyzer for your voice. Available as a
website and as a native desktop app.

## Architecture

All audio analysis is written in Rust and shared between both targets:

| Crate | Purpose |
|-------|---------|
| `crates/overtone-core` | FFT (Blackman window, Web-Audio-compatible byte scaling), pixel-row → frequency mapping, colouring, label strip, pitch detection. Emits a binary frame packet. |
| `crates/overtone-wasm` | `wasm-bindgen` wrapper used by the website. |
| `src-tauri` | Tauri desktop app: microphone capture with `cpal`, engine behind binary IPC. |

The React frontend (`src/`) never touches audio data. Each animation frame it
asks the active backend (`src/engine/`) for a frame packet and blits it onto
the canvases (`src/render/renderer.ts`).

* **Desktop**: `cpal` captures audio on a realtime thread → lock-free ring
  buffer → engine. The webview calls the `frame` command and receives raw
  bytes (`tauri::ipc::Response`).
* **Web**: an `AudioWorklet` captures PCM and posts it to the main thread,
  where the same engine (compiled to WebAssembly) does everything else.

## Development

Requirements: Node + pnpm, Rust (stable), `wasm-pack`, and the
`wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).

```sh
pnpm install
pnpm dev             # website (builds the wasm package first)
pnpm desktop:dev     # desktop app
pnpm test:core       # Rust unit tests for the analysis core
```

## Building

```sh
pnpm build           # website → dist/
pnpm desktop:build   # desktop bundles → src-tauri/target/release/bundle/
```

Manual probe of native audio capture without the GUI:

```sh
cargo run -p overtone-desktop --example capture_probe
```
