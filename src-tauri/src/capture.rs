//! Microphone capture with cpal. The audio callback only down-mixes to mono
//! and pushes into a lock-free SPSC ring buffer, so it never blocks on the
//! engine lock held by the render side.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{CachingCons, CachingProd, HeapRb};
use serde::Serialize;
use std::sync::Arc;

type Cons = CachingCons<Arc<HeapRb<f32>>>;
type Prod = CachingProd<Arc<HeapRb<f32>>>;

/// Ring capacity in mono samples (~2.7 s at 48 kHz, well above any FFT size).
const RING_CAPACITY: usize = 1 << 17;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Result<Vec<DeviceInfo>, String> {
    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string());
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for device in devices {
        let id = match device.id() {
            Ok(id) => id.to_string(),
            Err(_) => continue,
        };
        let name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| device.to_string());
        out.push(DeviceInfo {
            is_default: Some(&id) == default_id.as_ref(),
            id,
            name,
        });
    }
    Ok(out)
}

pub struct Capture {
    _stream: cpal::Stream,
    consumer: Cons,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl Capture {
    pub fn start(device_id: Option<&str>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = match device_id {
            Some(id) => {
                let id: cpal::DeviceId = id.parse().map_err(|e| format!("bad device id: {e:?}"))?;
                host.device_by_id(&id)
                    .ok_or_else(|| format!("input device {id} not found"))?
            }
            None => host
                .default_input_device()
                .ok_or_else(|| "no default input device".to_string())?,
        };
        let device_name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| device.to_string());
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels;
        let sample_rate = config.sample_rate;

        let (producer, consumer) = HeapRb::<f32>::new(RING_CAPACITY).split();

        let stream = match sample_format {
            cpal::SampleFormat::F32 => build::<f32>(&device, config, producer)?,
            cpal::SampleFormat::I16 => build::<i16>(&device, config, producer)?,
            cpal::SampleFormat::U16 => build::<u16>(&device, config, producer)?,
            cpal::SampleFormat::I32 => build::<i32>(&device, config, producer)?,
            cpal::SampleFormat::F64 => build::<f64>(&device, config, producer)?,
            other => return Err(format!("unsupported sample format {other}")),
        };
        stream.play().map_err(|e| e.to_string())?;

        Ok(Self {
            _stream: stream,
            consumer,
            sample_rate,
            channels,
            device_name,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Move captured mono samples into `out`; returns how many were written.
    pub fn drain(&mut self, out: &mut [f32]) -> usize {
        self.consumer.pop_slice(out)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut producer: Prod,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let scale = 1.0 / channels as f32;
    let mut mono: Vec<f32> = Vec::with_capacity(4096);
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _info| {
                mono.clear();
                if channels == 1 {
                    mono.extend(data.iter().map(|&s| f32::from_sample_(s)));
                } else {
                    mono.extend(data.chunks_exact(channels).map(|frame| {
                        frame.iter().map(|&s| f32::from_sample_(s)).sum::<f32>() * scale
                    }));
                }
                // If the consumer falls behind the oldest samples are dropped;
                // the analyser only ever looks at the most recent window anyway.
                let pushed = producer.push_slice(&mono);
                let _ = pushed;
            },
            |err| eprintln!("audio input error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
