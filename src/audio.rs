use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundId {
    BeaconTick,
    ProbeChirp,
    ProbeReply,
    AssocUp,
    DeauthZap,
    EapolMotif,
    RtsKnock,
    CtsKnockback,
    AckClick,
    DataTick,
    RetryGlitch,
}

#[derive(Clone)]
pub struct AudioHandle {
    queue: Arc<Mutex<VecDeque<f32>>>,
    palette: Arc<SoundPalette>,
}

pub struct AudioEngine {
    handle: AudioHandle,
    _stream: Stream,
}

#[derive(Clone)]
struct SoundPalette {
    sounds: std::collections::HashMap<SoundId, Vec<f32>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No default output device available")?;
        let config = device
            .default_output_config()
            .context("No default output config available")?;

        let sample_rate = config.sample_rate().0;
        let palette = Arc::new(build_palette(sample_rate));
        let queue = Arc::new(Mutex::new(VecDeque::with_capacity(4096)));
        let handle = AudioHandle {
            queue: Arc::clone(&queue),
            palette: palette.clone(),
        };

        let stream_config: StreamConfig = config.clone().into();
        let err_fn = |err| tracing::error!("Audio stream error: {err}");

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| write_samples_f32(data, &queue),
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| write_samples_i16(data, &queue),
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| write_samples_u16(data, &queue),
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow::anyhow!("Unsupported sample format: {:?}", other)),
        };

        stream.play()?;

        Ok(Self {
            handle,
            _stream: stream,
        })
    }

    pub fn handle(&self) -> AudioHandle {
        self.handle.clone()
    }
}

impl AudioHandle {
    pub fn play(&self, id: SoundId, overlay_retry: bool, gain: f32) {
        let mut guard = self.queue.lock().ok();
        if let Some(queue) = guard.as_mut() {
            if let Some(sound) = self.palette.sounds.get(&id) {
                for sample in sound.iter() {
                    queue.push_back(*sample * gain.clamp(0.0, 1.2));
                }
            }
            if overlay_retry {
                if let Some(glitch) = self.palette.sounds.get(&SoundId::RetryGlitch) {
                    for sample in glitch.iter() {
                        queue.push_back(*sample * gain.clamp(0.0, 1.2));
                    }
                }
            }
        }
    }
}

fn build_palette(sample_rate: u32) -> SoundPalette {
    use SoundId::*;
    let mut sounds = std::collections::HashMap::new();

    sounds.insert(BeaconTick, build_tick(sample_rate, 660.0, 30, 0.08));
    sounds.insert(ProbeChirp, build_tick(sample_rate, 1200.0, 24, 0.14));
    sounds.insert(ProbeReply, build_tick(sample_rate, 960.0, 34, 0.14));
    sounds.insert(AssocUp, build_blip(sample_rate, 520.0, 840.0, 50, 0.16));
    sounds.insert(DeauthZap, build_noise(sample_rate, 32, 0.4));
    sounds.insert(
        EapolMotif,
        build_motif(sample_rate, &[640.0, 760.0, 880.0, 1020.0], 22, 0.12),
    );
    sounds.insert(RtsKnock, build_tick(sample_rate, 360.0, 20, 0.12));
    sounds.insert(CtsKnockback, build_tick(sample_rate, 480.0, 20, 0.12));
    sounds.insert(AckClick, build_tick(sample_rate, 2200.0, 12, 0.04));
    sounds.insert(DataTick, build_tick(sample_rate, 820.0, 16, 0.07));
    sounds.insert(RetryGlitch, build_noise(sample_rate, 10, 0.05));

    SoundPalette { sounds }
}

fn build_tick(sample_rate: u32, freq_hz: f32, duration_ms: u64, volume: f32) -> Vec<f32> {
    let samples = ((sample_rate as u64 * duration_ms.max(1)) / 1000) as usize;
    let samples = samples.max(8);
    let envelope_len = (samples as f32 * 0.1).max(1.0) as usize;
    let mut data = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let env = if i < envelope_len {
            i as f32 / envelope_len as f32
        } else if i + envelope_len > samples {
            (samples - i) as f32 / envelope_len as f32
        } else {
            1.0
        };
        let val = (2.0 * PI * freq_hz * t).sin() * volume * env;
        data.push(val);
    }
    data
}

fn build_blip(
    sample_rate: u32,
    start_freq: f32,
    end_freq: f32,
    duration_ms: u64,
    volume: f32,
) -> Vec<f32> {
    let samples = ((sample_rate as u64 * duration_ms.max(1)) / 1000) as usize;
    let samples = samples.max(8);
    let mut data = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let progress = i as f32 / samples as f32;
        let freq = start_freq + (end_freq - start_freq) * progress;
        let env = (1.0 - progress * 0.6).max(0.2);
        let val = (2.0 * PI * freq * t).sin() * volume * env;
        data.push(val);
    }
    data
}

fn build_motif(sample_rate: u32, freqs: &[f32], note_ms: u64, volume: f32) -> Vec<f32> {
    let mut data = Vec::new();
    for &freq in freqs {
        let mut note = build_tick(sample_rate, freq, note_ms, volume);
        data.append(&mut note);
    }
    data
}

fn build_noise(sample_rate: u32, duration_ms: u64, volume: f32) -> Vec<f32> {
    let samples = ((sample_rate as u64 * duration_ms.max(1)) / 1000) as usize;
    let mut rng = rand::thread_rng();
    (0..samples)
        .map(|i| {
            let env = 1.0 - (i as f32 / samples as f32);
            (rand::Rng::r#gen::<f32>(&mut rng) * 2.0 - 1.0) * volume * env
        })
        .collect()
}

fn pop_sample(queue: &Arc<Mutex<VecDeque<f32>>>) -> f32 {
    queue
        .lock()
        .ok()
        .and_then(|mut q| q.pop_front())
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0)
}

fn write_samples_f32(data: &mut [f32], queue: &Arc<Mutex<VecDeque<f32>>>) {
    for sample in data.iter_mut() {
        *sample = pop_sample(queue);
    }
}

fn write_samples_i16(data: &mut [i16], queue: &Arc<Mutex<VecDeque<f32>>>) {
    for sample in data.iter_mut() {
        let v = pop_sample(queue);
        *sample = (v * i16::MAX as f32) as i16;
    }
}

fn write_samples_u16(data: &mut [u16], queue: &Arc<Mutex<VecDeque<f32>>>) {
    for sample in data.iter_mut() {
        let v = pop_sample(queue);
        *sample = ((v + 1.0) * 0.5 * u16::MAX as f32) as u16;
    }
}
