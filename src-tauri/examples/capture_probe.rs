//! Starts microphone capture for two seconds and reports what the engine
//! sees. Useful to verify device access outside of the GUI:
//! `cargo run -p overtone-desktop --example capture_probe`
use overtone_core::{Engine, EngineConfig, ViewRequest};
use std::time::{Duration, Instant};

fn main() {
    let devices = overtone_desktop_lib::capture::list_input_devices().expect("list devices");
    for d in &devices {
        println!(
            "device: {} {} {}",
            if d.is_default { "*" } else { " " },
            d.id,
            d.name
        );
    }
    let mut capture = overtone_desktop_lib::capture::Capture::start(None).expect("start capture");
    println!(
        "capturing from '{}' at {} Hz, {} channels",
        capture.device_name(),
        capture.sample_rate(),
        capture.channels()
    );
    let mut engine = Engine::new(EngineConfig {
        sample_rate: capture.sample_rate() as f64,
        ..Default::default()
    })
    .unwrap();
    let mut buf = vec![0.0f32; 1 << 16];
    let mut total = 0usize;
    let mut peak = 0.0f32;
    let start = Instant::now();
    let mut frames = 0;
    while start.elapsed() < Duration::from_secs(2) {
        loop {
            let n = capture.drain(&mut buf);
            if n == 0 {
                break;
            }
            total += n;
            peak = buf[..n].iter().fold(peak, |p, s| p.max(s.abs()));
            engine.push_mono(&buf[..n]);
        }
        let t = Instant::now();
        let packet_len = engine
            .frame(ViewRequest {
                width: 2048,
                px_per_second: 120.0,
                view_end: None,
            })
            .len();
        frames += 1;
        if frames % 30 == 0 {
            let p = engine.last_pitch();
            println!(
                "frame {} bytes, {:?} per frame, pitch {:.1} Hz",
                packet_len,
                t.elapsed(),
                p.hz
            );
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    println!("captured {total} samples in 2 s (peak {peak:.4}), {frames} frames");
}
