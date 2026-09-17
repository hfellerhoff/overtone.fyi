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

The engine analyses the signal with the window size chosen under
"Resolution" and records each spectrum in a ring buffer (256 MB by default,
about 20 minutes at the default settings), independent of the display. The
engine also supports splitting the range into bands with shorter windows for
the treble (`BandPreset::Balanced` and `Sharp` in
`crates/overtone-core/src/spectrum.rs`); the apps use a single band.

The raw audio is retained as well (64 MB, about six minutes). When an
analysis setting changes (resolution, bands, sample rate) the newest few
seconds are re-analysed immediately and the rest is filled in over the
following frames, so the display carries over instead of being cleared.
The canvas is only ever a rendering of the recorded analysis. The React frontend (`src/`)
never touches audio data. Each animation frame it asks the active backend
(`src/engine/`) for the view it wants (timeline width, pixels per second,
and where in time the newest edge sits) and blits the returned packet onto
the canvases (`src/render/renderer.ts`). The engine sends only the columns
that changed, or a full redraw after a zoom.

## Controls

| Input | Action |
|-------|--------|
| Space | Start / stop capture |
| Wheel or pinch over the spectrogram | Zoom the frequency range around the cursor |
| Shift + wheel, or horizontal wheel / swipe | Scroll backward and forward in time |
| Alt + wheel | Pan the frequency range |
| Esc or 0, or the "back to live" button | Reset zoom and return to live |

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
