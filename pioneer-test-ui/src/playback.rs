//! Desktop audio preview playback (CoreAudio HAL via cpal on macOS).
//!
//! Architecture: symphonia decodes on a worker thread → `ringbuf` producer (Mutex OK off RT thread).
//! cpal output callback only `try_pop`s from the consumer — no locks, no blocking, no alloc.

use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};
use cpal::{
    BufferSize, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use serde::Serialize;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tauri::{AppHandle, Emitter};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_BUFFER_FRAMES: u32 = 128;
/// ~2s stereo @ 48 kHz — back-pressure on decode thread if the ring fills.
const RING_SAMPLES: usize = 48_000 * 2 * 2;
/// Crossfade length applied at every stream switch (play/seek/stop). Ramps from
/// the last sample played into the new position so the waveform discontinuity
/// doesn't produce an audible click/pop. ~5ms at 48 kHz.
const DECLICK_FRAMES: usize = 256;
/// Max channels tracked for the declick crossfade seam.
const MAX_CH: usize = 8;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PlaybackAudioConfig {
    pub sample_rate: u32,
    pub buffer_frames: u32,
}

impl Default for PlaybackAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            buffer_frames: DEFAULT_BUFFER_FRAMES,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct PlaybackTick {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub playing: bool,
    /// Stereo master peaks 0..1 (VU ballistics applied before emit).
    pub level_left: f32,
    pub level_right: f32,
}

#[derive(Clone)]
struct SamplePipe {
    producer: Arc<Mutex<HeapProd<f32>>>,
    /// Bumped on each play/seek so the RT callback clears stale samples.
    stream_generation: Arc<AtomicU64>,
    ended: Arc<AtomicBool>,
    decode_generation: Arc<AtomicU64>,
    /// Frames the output callback has actually consumed since the last
    /// play/seek. Reset by the RT callback when it sees a new stream
    /// generation. The tick thread derives playback position from this
    /// (audio clock) so the playhead can't run ahead of the audio during the
    /// decode/output startup latency.
    frames_played: Arc<AtomicU64>,
}

fn new_sample_pipe() -> (SamplePipe, HeapCons<f32>) {
    let rb = HeapRb::<f32>::new(RING_SAMPLES);
    let (prod, cons) = rb.split();
    let pipe = SamplePipe {
        producer: Arc::new(Mutex::new(prod)),
        stream_generation: Arc::new(AtomicU64::new(0)),
        ended: Arc::new(AtomicBool::new(true)),
        decode_generation: Arc::new(AtomicU64::new(0)),
        frames_played: Arc::new(AtomicU64::new(0)),
    };
    (pipe, cons)
}

fn prepare_pipe_for_play(pipe: &SamplePipe) {
    pipe.stream_generation.fetch_add(1, Ordering::SeqCst);
    pipe.ended.store(false, Ordering::Relaxed);
}

struct PlaybackState {
    path: Option<String>,
    duration_ms: u64,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    pipe: SamplePipe,
    channels: u16,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
}

struct OutputDeviceInfo {
    channels: u16,
    sample_rate: u32,
}

#[allow(dead_code)]
pub struct AudioStreamHandle(pub cpal::Stream);
unsafe impl Send for AudioStreamHandle {}
unsafe impl Sync for AudioStreamHandle {}

pub struct PlaybackEngine {
    audio_config: Mutex<PlaybackAudioConfig>,
    output_device_name: Mutex<Option<String>>,
    output: Mutex<Option<OutputDeviceInfo>>,
    output_epoch: Arc<AtomicU64>,
    state: Mutex<PlaybackState>,
    active_stream: Mutex<Option<AudioStreamHandle>>,
}

#[derive(Clone, Serialize)]
pub struct AudioOutputDevice {
    pub name: String,
    pub is_default: bool,
}

impl PlaybackEngine {
    pub fn new() -> Result<Self, String> {
        let config = PlaybackAudioConfig::default();
        let (pipe, _cons) = new_sample_pipe();
        let engine = Self {
            audio_config: Mutex::new(config),
            output_device_name: Mutex::new(None),
            output: Mutex::new(None),
            output_epoch: Arc::new(AtomicU64::new(0)),
            state: Mutex::new(PlaybackState {
                path: None,
                duration_ms: 0,
                position_ms: Arc::new(AtomicU64::new(0)),
                playing: Arc::new(AtomicBool::new(false)),
                tick_stop: Arc::new(AtomicBool::new(true)),
                pipe,
                channels: 2,
                level_left: Arc::new(AtomicU32::new(0)),
                level_right: Arc::new(AtomicU32::new(0)),
            }),
            active_stream: Mutex::new(None),
        };
        engine.rebuild_output_stream(config)?;
        Ok(engine)
    }

    pub fn list_output_devices(&self) -> Result<Vec<AudioOutputDevice>, String> {
        list_output_devices()
    }

    pub fn output_device_name(&self) -> Result<Option<String>, String> {
        Ok(self
            .output_device_name
            .lock()
            .map_err(|e| e.to_string())?
            .clone())
    }

    pub fn set_output_device(&self, name: Option<String>) -> Result<(), String> {
        self.stop()?;
        *self
            .output_device_name
            .lock()
            .map_err(|e| e.to_string())? = name;
        let config = *self.audio_config.lock().map_err(|e| e.to_string())?;
        self.rebuild_output_stream(config)
    }

    pub fn configure(&self, sample_rate: u32, buffer_frames: u32) -> Result<PlaybackAudioConfig, String> {
        let sample_rate = sample_rate.clamp(8_000, 192_000);
        let buffer_frames = buffer_frames.clamp(32, 4096);
        let config = PlaybackAudioConfig {
            sample_rate,
            buffer_frames,
        };
        self.stop()?;
        *self.audio_config.lock().map_err(|e| e.to_string())? = config;
        self.rebuild_output_stream(config)?;
        Ok(config)
    }

    pub fn audio_config(&self) -> Result<PlaybackAudioConfig, String> {
        Ok(*self.audio_config.lock().map_err(|e| e.to_string())?)
    }

    fn rebuild_output_stream(&self, config: PlaybackAudioConfig) -> Result<(), String> {
        let device = resolve_output_device(
            self.output_device_name
                .lock()
                .map_err(|e| e.to_string())?
                .as_deref(),
        )?;

        let (supported, stream_config) =
            resolve_stream_config(&device, config.sample_rate, config.buffer_frames)?;

        let stream_epoch = self.output_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let (pipe, mut cons) = new_sample_pipe();
        let stream_generation = pipe.stream_generation.clone();
        let frames_played = pipe.frames_played.clone();
        let ended = pipe.ended.clone();
        let level_left;
        let level_right;
        {
            let mut g = self.state.lock().map_err(|e| e.to_string())?;
            g.pipe = pipe;
            level_left = g.level_left.clone();
            level_right = g.level_right.clone();
        }
        let channels = stream_config.channels;
        let active_epoch = self.output_epoch.clone();
        let mut callback_generation = 0u64;
        let mut declick_remaining = 0usize;
        let mut seam = [0f32; MAX_CH];
        let mut last_sample = [0f32; MAX_CH];

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| {
                    if active_epoch.load(Ordering::SeqCst) != stream_epoch {
                        for s in data.iter_mut() {
                            *s = 0.0;
                        }
                        return;
                    }
                    fill_output_buffer(
                        data,
                        channels,
                        &mut cons,
                        &stream_generation,
                        &mut callback_generation,
                        &ended,
                        &frames_played,
                        &level_left,
                        &level_right,
                        &mut declick_remaining,
                        &mut seam,
                        &mut last_sample,
                    );
                },
                |err| eprintln!("audio output stream error: {err}"),
                None,
            )
            .map_err(|e| format!("Failed to open CoreAudio output stream: {e}"))?;

        stream.play().map_err(|e| e.to_string())?;
        *self.active_stream.lock().map_err(|e| e.to_string())? = Some(AudioStreamHandle(stream));

        *self.output.lock().map_err(|e| e.to_string())? = Some(OutputDeviceInfo {
            channels,
            sample_rate: supported.sample_rate().0,
        });

        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        g.channels = channels;
        Ok(())
    }

    pub fn play(
        &self,
        app: &AppHandle,
        path: &str,
        position_ms: u64,
        duration_ms_hint: Option<u64>,
    ) -> Result<(), String> {
        let output = self.output.lock().map_err(|e| e.to_string())?;
        let output = output.as_ref().ok_or_else(|| "Audio output not initialized".to_string())?;
        let sample_rate = output.sample_rate;
        let channels = output.channels;

        let (duration_ms, start_sample) = match duration_ms_hint.filter(|d| *d > 0) {
            Some(duration_ms) => {
                let ch = channels.max(1) as u128;
                let start_sample = (position_ms as u128)
                    .saturating_mul(sample_rate as u128)
                    .saturating_mul(ch)
                    / 1000;
                (duration_ms, start_sample as u64)
            }
            None => decode_start_offset(path, position_ms, sample_rate, channels)?,
        };

        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        stop_decode_locked(&mut g);

        let generation = g.pipe.decode_generation.fetch_add(1, Ordering::SeqCst) + 1;
        prepare_pipe_for_play(&g.pipe);
        let start_position_ms = position_ms.min(duration_ms);
        g.path = Some(path.to_string());
        g.duration_ms = duration_ms;
        g.position_ms.store(start_position_ms, Ordering::SeqCst);
        g.playing.store(true, Ordering::SeqCst);

        // Fresh stop flag per tick thread. stop_decode_locked() just set the
        // previous generation's flag to true; replacing the Arc (instead of
        // resetting it to false) guarantees the old tick thread sees `true` and
        // exits. Sharing one flag let play()/seek() reset it before the old
        // thread woke from its 50ms sleep, leaving zombie threads that each
        // emitted ticks at their own captured start position (flickering
        // playheads, one per scrub).
        let tick_stop = Arc::new(AtomicBool::new(false));
        g.tick_stop = tick_stop.clone();
        spawn_decode_thread(
            path.to_string(),
            start_sample,
            sample_rate,
            channels,
            g.pipe.clone(),
            generation,
        );
        spawn_tick_thread(
            app.clone(),
            g.position_ms.clone(),
            g.playing.clone(),
            tick_stop,
            duration_ms,
            start_position_ms,
            sample_rate,
            g.pipe.frames_played.clone(),
            g.level_left.clone(),
            g.level_right.clone(),
        );

        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        g.playing.store(false, Ordering::SeqCst);
        stop_decode_locked(&mut g);
        Ok(())
    }

    pub fn resume(&self, app: &AppHandle) -> Result<(), String> {
        let output = self.output.lock().map_err(|e| e.to_string())?;
        let output = output.as_ref().ok_or_else(|| "Audio output not initialized".to_string())?;
        let sample_rate = output.sample_rate;
        let channels = output.channels;

        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        let Some(path) = g.path.clone() else {
            return Ok(());
        };
        let position_ms = g.position_ms.load(Ordering::SeqCst);
        let duration_ms = g.duration_ms;
        if position_ms >= duration_ms && duration_ms > 0 {
            return Ok(());
        }

        stop_decode_locked(&mut g);
        let generation = g.pipe.decode_generation.fetch_add(1, Ordering::SeqCst) + 1;
        prepare_pipe_for_play(&g.pipe);
        g.playing.store(true, Ordering::SeqCst);

        let start_sample = (position_ms as u128)
            .saturating_mul(sample_rate as u128)
            .saturating_mul(channels.max(1) as u128)
            / 1000;

        spawn_decode_thread(
            path,
            start_sample as u64,
            sample_rate,
            channels,
            g.pipe.clone(),
            generation,
        );
        // Fresh stop flag per tick thread (see play() for why a shared flag
        // leaves zombie tick threads).
        let tick_stop = Arc::new(AtomicBool::new(false));
        g.tick_stop = tick_stop.clone();
        spawn_tick_thread(
            app.clone(),
            g.position_ms.clone(),
            g.playing.clone(),
            tick_stop,
            duration_ms,
            position_ms,
            sample_rate,
            g.pipe.frames_played.clone(),
            g.level_left.clone(),
            g.level_right.clone(),
        );
        Ok(())
    }

    pub fn set_position(&self, position_ms: u64) -> Result<(), String> {
        let g = self.state.lock().map_err(|e| e.to_string())?;
        let clamped = if g.duration_ms > 0 {
            position_ms.min(g.duration_ms)
        } else {
            position_ms
        };
        g.position_ms.store(clamped, Ordering::SeqCst);
        Ok(())
    }

    pub fn seek(&self, app: &AppHandle, position_ms: u64) -> Result<(), String> {
        let (path, duration_ms) = {
            let g = self.state.lock().map_err(|e| e.to_string())?;
            (g.path.clone(), g.duration_ms)
        };
        let Some(path) = path else {
            return Ok(());
        };
        // Reuse the duration established at play() so the playhead/beat-grid
        // timebase stays stable across scrubs (don't re-derive it by decoding,
        // which can return a slightly different length).
        let hint = (duration_ms > 0).then_some(duration_ms);
        self.play(app, &path, position_ms, hint)
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut g = self.state.lock().map_err(|e| e.to_string())?;
        stop_decode_locked(&mut g);
        g.path = None;
        g.duration_ms = 0;
        g.position_ms.store(0, Ordering::SeqCst);
        g.playing.store(false, Ordering::SeqCst);
        g.level_left.store(0, Ordering::Relaxed);
        g.level_right.store(0, Ordering::Relaxed);
        Ok(())
    }
}

fn resolve_stream_config(
    device: &cpal::Device,
    sample_rate: u32,
    buffer_frames: u32,
) -> Result<(SupportedStreamConfig, StreamConfig), String> {
    let target = SampleRate(sample_rate);
    let mut ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| e.to_string())?
        .filter(|r| r.sample_format() == SampleFormat::F32)
        .collect();
    ranges.sort_by(|a, b| b.cmp_default_heuristics(a));

    for range in ranges {
        if let Some(supported) = range.clone().try_with_sample_rate(target) {
            let stream_config = stream_config_with_buffer(&supported, buffer_frames);
            return Ok((supported, stream_config));
        }
    }

    let default = device
        .default_output_config()
        .map_err(|e| e.to_string())?;
    if default.sample_format() != SampleFormat::F32 {
        return Err("Default output device does not support f32 samples".to_string());
    }
    let stream_config = stream_config_with_buffer(&default, buffer_frames);
    Ok((default, stream_config))
}

fn stream_config_with_buffer(supported: &SupportedStreamConfig, buffer_frames: u32) -> StreamConfig {
    let mut stream_config = supported.config();
    let frames = match supported.buffer_size() {
        SupportedBufferSize::Range { min, max } => buffer_frames.clamp(*min, *max),
        SupportedBufferSize::Unknown => buffer_frames,
    };
    stream_config.buffer_size = BufferSize::Fixed(frames);
    stream_config
}

fn store_f32(atom: &AtomicU32, v: f32) {
    atom.store(v.to_bits(), Ordering::Relaxed);
}

fn load_f32(atom: &AtomicU32) -> f32 {
    f32::from_bits(atom.load(Ordering::Relaxed))
}

/// Fast attack on peaks from the output callback; release happens on the tick thread.
fn attack_meter_levels(data: &[f32], channels: u16, left: &AtomicU32, right: &AtomicU32) {
    let ch = channels.max(1) as usize;
    let mut pk_l = 0.0f32;
    let mut pk_r = 0.0f32;
    for frame in data.chunks(ch) {
        pk_l = pk_l.max(frame[0].abs());
        pk_r = if ch > 1 {
            pk_r.max(frame[1].abs())
        } else {
            pk_l
        };
    }
    let cur_l = load_f32(left);
    let cur_r = load_f32(right);
    store_f32(left, cur_l.max(pk_l).min(1.0));
    store_f32(right, cur_r.max(pk_r).min(1.0));
}

fn release_meter_levels(left: &AtomicU32, right: &AtomicU32, factor: f32) {
    let l = load_f32(left) * factor;
    let r = load_f32(right) * factor;
    store_f32(left, if l < 0.004 { 0.0 } else { l });
    store_f32(right, if r < 0.004 { 0.0 } else { r });
}

#[allow(clippy::too_many_arguments)]
fn fill_output_buffer(
    data: &mut [f32],
    channels: u16,
    cons: &mut HeapCons<f32>,
    stream_generation: &AtomicU64,
    callback_generation: &mut u64,
    ended: &AtomicBool,
    frames_played: &AtomicU64,
    level_left: &AtomicU32,
    level_right: &AtomicU32,
    declick_remaining: &mut usize,
    seam: &mut [f32; MAX_CH],
    last_sample: &mut [f32; MAX_CH],
) {
    let stream_gen = stream_generation.load(Ordering::Acquire);
    if stream_gen != *callback_generation {
        cons.clear();
        // New play/seek/stop: restart the audio clock so the tick thread measures
        // position from the first sample this stream actually outputs.
        frames_played.store(0, Ordering::Relaxed);
        *callback_generation = stream_gen;
        // Start a crossfade from the last sample we played into the new stream so
        // the position discontinuity doesn't click. If the new decode thread
        // hasn't produced samples yet (underrun), this fades the seam out to
        // silence instead — which also declicks pause/stop.
        *seam = *last_sample;
        *declick_remaining = DECLICK_FRAMES;
    }

    let ch = channels.max(1) as usize;
    let ramp = DECLICK_FRAMES as f32;
    let mut popped = 0u64;

    for frame in data.chunks_mut(ch) {
        // Crossfade gain for this frame: out = seam*(1-t) + new*t, t: 0 → 1.
        let (mix_old, mix_new) = if *declick_remaining > 0 {
            let t = 1.0 - (*declick_remaining as f32 / ramp);
            (1.0 - t, t)
        } else {
            (0.0, 1.0)
        };
        for (c, slot) in frame.iter_mut().enumerate() {
            let v = match cons.try_pop() {
                Some(v) => {
                    popped += 1;
                    v
                }
                // Underrun (ring empty): treat new sample as silence but don't
                // advance the audio clock — position only moves for samples we
                // genuinely played.
                None => 0.0,
            };
            let seam_v = if c < MAX_CH { seam[c] } else { 0.0 };
            let out = v * mix_new + seam_v * mix_old;
            *slot = out;
            if c < MAX_CH {
                last_sample[c] = out;
            }
        }
        if *declick_remaining > 0 {
            *declick_remaining -= 1;
        }
    }

    if popped > 0 {
        frames_played.fetch_add(popped / ch as u64, Ordering::Relaxed);
    }
    attack_meter_levels(data, channels, level_left, level_right);
    let _ = ended;
}

fn stop_decode_locked(g: &mut PlaybackState) {
    g.pipe.decode_generation.fetch_add(1, Ordering::SeqCst);
    // Bump the stream generation so the RT callback flushes any already-decoded
    // samples still in the ring (~2s). Without this, pause/stop have a long
    // audible tail while the buffer drains. play() bumps it again right after.
    g.pipe.stream_generation.fetch_add(1, Ordering::SeqCst);
    g.pipe.ended.store(true, Ordering::Relaxed);
    g.tick_stop.store(true, Ordering::SeqCst);
}

fn list_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok());
    let mut out = Vec::new();
    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;
    for device in devices {
        let name = device.name().unwrap_or_else(|_| "Unknown device".to_string());
        let is_default = default_name.as_ref() == Some(&name);
        if out.iter().any(|d: &AudioOutputDevice| d.name == name) {
            continue;
        }
        out.push(AudioOutputDevice { name, is_default });
    }
    out.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

fn resolve_output_device(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if let Some(name) = name.filter(|n| !n.is_empty()) {
        let devices = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;
        for device in devices {
            if device.name().ok().as_deref() == Some(name) {
                return Ok(device);
            }
        }
        eprintln!("Warning: Configured output device '{}' not found. Falling back to system default.", name);
    }
    host.default_output_device()
        .ok_or_else(|| "No default output audio device".to_string())
}

fn decode_start_offset(
    path: &str,
    position_ms: u64,
    sample_rate: u32,
    channels: u16,
) -> Result<(u64, u64), String> {
    let file = File::open(path).map_err(|e| format!("Open {path}: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let hint = Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| e.to_string())?;
    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "No default audio track".to_string())?;
    let tb = track
        .codec_params
        .time_base
        .or(track.codec_params.sample_rate.map(|sr| {
            symphonia::core::units::TimeBase {
                numer: 1,
                denom: sr,
            }
        }))
        .ok_or_else(|| "Unknown track duration".to_string())?;
    let duration_frames = track.codec_params.n_frames.unwrap_or(0);
    let duration_ms = if duration_frames > 0 {
        tb.calc_time(duration_frames).seconds as u64 * 1000
            + (tb.calc_time(duration_frames).frac * 1000.0) as u64
    } else {
        0
    };
    let ch = channels.max(1) as u64;
    let start_sample = (position_ms as u128)
        .saturating_mul(sample_rate as u128)
        .saturating_mul(ch as u128)
        / 1000;
    Ok((duration_ms, start_sample as u64))
}

fn push_interleaved(
    producer: &Mutex<HeapProd<f32>>,
    decode_generation: &AtomicU64,
    generation: u64,
    samples: &[f32],
) -> Result<bool, String> {
    for &s in samples {
        loop {
            if decode_generation.load(Ordering::Relaxed) != generation {
                return Ok(false);
            }
            let mut prod = producer.lock().map_err(|e| e.to_string())?;
            if prod.try_push(s).is_ok() {
                break;
            }
            drop(prod);
            thread::sleep(Duration::from_micros(250));
        }
    }
    Ok(true)
}

fn spawn_decode_thread(
    path: String,
    start_sample: u64,
    sample_rate: u32,
    channels: u16,
    pipe: SamplePipe,
    generation: u64,
) {
    thread::spawn(move || {
        if let Err(e) = run_decode(
            &path,
            start_sample,
            sample_rate,
            channels,
            &pipe,
            generation,
        ) {
            eprintln!("playback decode error ({path}): {e}");
            if pipe.decode_generation.load(Ordering::Relaxed) == generation {
                pipe.ended.store(true, Ordering::Relaxed);
            }
        }
    });
}

fn run_decode(
    path: &str,
    start_sample: u64,
    out_rate: u32,
    out_channels: u16,
    pipe: &SamplePipe,
    generation: u64,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("Open {path}: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let hint = Hint::new();
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;
    let default = format
        .default_track()
        .ok_or_else(|| "No default audio track".to_string())?;
    let track_id = default.id;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.id == track_id)
        .ok_or_else(|| "Default audio track missing".to_string())?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let track_params = &track.codec_params;
    let in_rate = track_params.sample_rate.unwrap_or(out_rate);
    let in_channels = track_params.channels.map(|c| c.count()).unwrap_or(2).max(1);
    let time_base = track_params.time_base;

    let target_start = start_sample;
    let mut samples_out: u64 = 0;

    // Seek the demuxer near the target instead of decoding-and-discarding every
    // sample before it. Without this, seeking deep into a track silently decodes
    // everything before the target (multi-second gap → playback appears to stop).
    if target_start > 0 && out_channels > 0 {
        let target_seconds = start_sample as f64 / (out_rate as f64 * out_channels as f64);
        match format.seek(
            symphonia::core::formats::SeekMode::Accurate,
            symphonia::core::formats::SeekTo::Time {
                time: symphonia::core::units::Time::from(target_seconds),
                track_id: Some(track_id),
            },
        ) {
            Ok(seeked) => {
                decoder.reset();
                let actual_seconds = match time_base {
                    Some(tb) => {
                        let t = tb.calc_time(seeked.actual_ts);
                        t.seconds as f64 + t.frac
                    }
                    None => seeked.actual_ts as f64 / in_rate as f64,
                };
                // Account for samples the seek already skipped; the loop below
                // trims only the sub-packet remainder up to target_start.
                samples_out =
                    (actual_seconds * out_rate as f64).round() as u64 * out_channels as u64;
            }
            // Seek unsupported for this source — fall back to decode-and-skip.
            Err(_) => samples_out = 0,
        }
    }

    loop {
        if pipe.decode_generation.load(Ordering::Relaxed) != generation {
            return Ok(());
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(|e| e.to_string())?;
        let mut frame_samples = audio_buffer_to_interleaved(&decoded);
        if in_rate != out_rate {
            frame_samples = resample_interleaved(&frame_samples, in_rate, out_rate, in_channels);
        }
        frame_samples = to_output_channels(&frame_samples, in_channels, out_channels as usize);

        for chunk in frame_samples.chunks(out_channels as usize) {
            if samples_out + chunk.len() as u64 <= target_start {
                samples_out += chunk.len() as u64;
                continue;
            }
            if !push_interleaved(
                &pipe.producer,
                &pipe.decode_generation,
                generation,
                chunk,
            )? {
                return Ok(());
            }
        }
    }

    if pipe.decode_generation.load(Ordering::Relaxed) == generation {
        pipe.ended.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Convert any decoded Symphonia buffer to interleaved f32.
/// Uses `SampleBuffer::copy_interleaved_ref` so every PCM sample format
/// (U8/S16/S24/S32/F32/F64…) is handled — not just a hand-picked few.
fn audio_buffer_to_interleaved(buf: &AudioBufferRef<'_>) -> Vec<f32> {
    let spec = *buf.spec();
    let duration = buf.capacity() as u64;
    if duration == 0 {
        return Vec::new();
    }
    let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
    sample_buf.copy_interleaved_ref(buf.clone());
    sample_buf.samples().to_vec()
}

fn resample_interleaved(input: &[f32], from_rate: u32, to_rate: u32, channels: usize) -> Vec<f32> {
    if from_rate == to_rate || channels == 0 {
        return input.to_vec();
    }
    let frames_in = input.len() / channels;
    if frames_in == 0 {
        return Vec::new();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let frames_out = ((frames_in as f64) * ratio).ceil() as usize;
    let mut out = Vec::with_capacity(frames_out * channels);
    for fo in 0..frames_out {
        let src = fo as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let idx_next = (idx + 1).min(frames_in.saturating_sub(1));
        for c in 0..channels {
            let a = input[idx * channels + c];
            let b = input[idx_next * channels + c];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

fn to_output_channels(samples: &[f32], in_channels: usize, out_channels: usize) -> Vec<f32> {
    if in_channels == 0 || out_channels == 0 {
        return Vec::new();
    }
    if in_channels == out_channels {
        return samples.to_vec();
    }
    let frames = samples.len() / in_channels;
    let mut out = Vec::with_capacity(frames * out_channels);
    for f in 0..frames {
        let base = f * in_channels;
        let l = samples[base];
        let r = if in_channels > 1 {
            samples[base + 1]
        } else {
            l
        };
        match out_channels {
            1 => out.push((l + r) * 0.5),
            _ => {
                out.push(l);
                out.push(r);
                for _ in 2..out_channels {
                    out.push(0.0);
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn spawn_tick_thread(
    app: AppHandle,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    duration_ms: u64,
    start_position_ms: u64,
    sample_rate: u32,
    frames_played: Arc<AtomicU64>,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
) {
    thread::spawn(move || {
        let rate = sample_rate.max(1) as u64;
        while !tick_stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
            if playing.load(Ordering::SeqCst) {
                // Position follows the audio clock: how many frames the output
                // callback has actually played since this play/seek started.
                let played_ms = frames_played.load(Ordering::Relaxed) * 1000 / rate;
                let next = (start_position_ms + played_ms).min(duration_ms);
                position_ms.store(next, Ordering::SeqCst);
                if duration_ms > 0 && next >= duration_ms {
                    playing.store(false, Ordering::SeqCst);
                }
                release_meter_levels(&level_left, &level_right, 0.78);
            } else {
                release_meter_levels(&level_left, &level_right, 0.55);
            }
            let tick = PlaybackTick {
                position_ms: position_ms.load(Ordering::SeqCst),
                duration_ms,
                playing: playing.load(Ordering::SeqCst),
                level_left: load_f32(&level_left),
                level_right: load_f32(&level_right),
            };
            let _ = app.emit("playback-tick", tick);
        }
    });
}

#[tauri::command]
pub fn playback_configure(
    sample_rate: u32,
    buffer_frames: u32,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<PlaybackAudioConfig, String> {
    engine.configure(sample_rate, buffer_frames)
}

#[tauri::command]
pub fn playback_get_config(
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<PlaybackAudioConfig, String> {
    engine.audio_config()
}

#[tauri::command]
pub fn playback_play(
    path: String,
    position_ms: u64,
    duration_ms: Option<u64>,
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.play(&app, &path, position_ms, duration_ms)
}

#[tauri::command]
pub fn playback_pause(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.pause()
}

#[tauri::command]
pub fn playback_resume(
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.resume(&app)
}

#[tauri::command]
pub fn playback_seek(
    position_ms: u64,
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.seek(&app, position_ms)
}

#[tauri::command]
pub fn playback_set_position(
    position_ms: u64,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.set_position(position_ms)
}

#[tauri::command]
pub fn playback_stop(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.stop()
}

#[tauri::command]
pub fn playback_list_output_devices(
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<Vec<AudioOutputDevice>, String> {
    engine.list_output_devices()
}

#[tauri::command]
pub fn playback_get_output_device(
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<Option<String>, String> {
    engine.output_device_name()
}

#[tauri::command]
pub fn playback_set_output_device(
    name: Option<String>,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.set_output_device(name)
}
