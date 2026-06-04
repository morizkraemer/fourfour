//! Desktop audio preview playback (CoreAudio via cpal on macOS).

use std::collections::VecDeque;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use serde::Serialize;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tauri::{AppHandle, Emitter};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_BUFFER_FRAMES: u32 = 128;

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

struct SampleQueue {
    samples: VecDeque<f32>,
    ended: bool,
    generation: u64,
}

struct PlaybackState {
    path: Option<String>,
    duration_ms: u64,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    queue: Arc<Mutex<SampleQueue>>,
    queue_cv: Arc<Condvar>,
    decode_generation: u64,
    channels: u16,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
}

struct OutputDeviceInfo {
    channels: u16,
    sample_rate: u32,
}

pub struct PlaybackEngine {
    audio_config: Mutex<PlaybackAudioConfig>,
    output: Mutex<Option<OutputDeviceInfo>>,
    output_epoch: Arc<AtomicU64>,
    state: Mutex<PlaybackState>,
}

impl PlaybackEngine {
    pub fn new() -> Result<Self, String> {
        let config = PlaybackAudioConfig::default();
        let engine = Self {
            audio_config: Mutex::new(config),
            output: Mutex::new(None),
            output_epoch: Arc::new(AtomicU64::new(0)),
            state: Mutex::new(PlaybackState {
                path: None,
                duration_ms: 0,
                position_ms: Arc::new(AtomicU64::new(0)),
                playing: Arc::new(AtomicBool::new(false)),
                tick_stop: Arc::new(AtomicBool::new(true)),
                queue: Arc::new(Mutex::new(SampleQueue {
                    samples: VecDeque::new(),
                    ended: false,
                    generation: 0,
                })),
                queue_cv: Arc::new(Condvar::new()),
                decode_generation: 0,
                channels: 2,
                level_left: Arc::new(AtomicU32::new(0)),
                level_right: Arc::new(AtomicU32::new(0)),
            }),
        };
        engine.rebuild_output_stream(config)?;
        Ok(engine)
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
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| "No default output audio device".to_string())?;

        let (supported, stream_config) =
            resolve_stream_config(&device, config.sample_rate, config.buffer_frames)?;

        let stream_epoch = self.output_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let queue = state.queue.clone();
        let queue_cv = state.queue_cv.clone();
        let level_left = state.level_left.clone();
        let level_right = state.level_right.clone();
        let channels = stream_config.channels;
        let active_epoch = self.output_epoch.clone();

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
                        &queue,
                        &queue_cv,
                        &level_left,
                        &level_right,
                    );
                },
                |err| eprintln!("audio output stream error: {err}"),
                None,
            )
            .map_err(|e| format!("Failed to open CoreAudio output stream: {e}"))?;

        stream.play().map_err(|e| e.to_string())?;
        // cpal::Stream is !Send; leak it so PlaybackEngine stays Send + Sync for Tauri.
        let _leaked: &'static cpal::Stream = Box::leak(Box::new(stream));

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

        g.decode_generation = g.decode_generation.wrapping_add(1);
        let generation = g.decode_generation;
        g.path = Some(path.to_string());
        g.duration_ms = duration_ms;
        g.position_ms.store(position_ms.min(duration_ms), Ordering::SeqCst);
        g.playing.store(true, Ordering::SeqCst);

        {
            let mut q = g.queue.lock().map_err(|e| e.to_string())?;
            q.samples.clear();
            q.ended = false;
            q.generation = generation;
        }

        g.tick_stop.store(false, Ordering::SeqCst);
        spawn_decode_thread(
            path.to_string(),
            start_sample,
            sample_rate,
            channels,
            g.queue.clone(),
            g.queue_cv.clone(),
            generation,
        );
        spawn_tick_thread(
            app.clone(),
            g.position_ms.clone(),
            g.playing.clone(),
            g.tick_stop.clone(),
            duration_ms,
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
        g.decode_generation = g.decode_generation.wrapping_add(1);
        let generation = g.decode_generation;
        g.playing.store(true, Ordering::SeqCst);

        let start_sample = (position_ms as u128)
            .saturating_mul(sample_rate as u128)
            .saturating_mul(channels.max(1) as u128)
            / 1000;

        {
            let mut q = g.queue.lock().map_err(|e| e.to_string())?;
            q.samples.clear();
            q.ended = false;
            q.generation = generation;
        }

        spawn_decode_thread(
            path,
            start_sample as u64,
            sample_rate,
            channels,
            g.queue.clone(),
            g.queue_cv.clone(),
            generation,
        );
        g.tick_stop.store(false, Ordering::SeqCst);
        spawn_tick_thread(
            app.clone(),
            g.position_ms.clone(),
            g.playing.clone(),
            g.tick_stop.clone(),
            duration_ms,
            g.level_left.clone(),
            g.level_right.clone(),
        );
        Ok(())
    }

    pub fn seek(&self, app: &AppHandle, position_ms: u64) -> Result<(), String> {
        let path = {
            let g = self.state.lock().map_err(|e| e.to_string())?;
            g.path.clone()
        };
        let Some(path) = path else {
            return Ok(());
        };
        self.play(app, &path, position_ms, None)
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

fn fill_output_buffer(
    data: &mut [f32],
    channels: u16,
    queue: &Arc<Mutex<SampleQueue>>,
    queue_cv: &Arc<Condvar>,
    level_left: &AtomicU32,
    level_right: &AtomicU32,
) {
    let ch = channels as usize;
    let mut out_idx = 0;
    while out_idx < data.len() {
        let mut q = queue.lock().unwrap();
        if q.samples.len() < ch {
            if q.ended {
                for slot in &mut data[out_idx..] {
                    *slot = 0.0;
                }
                return;
            }
            q = queue_cv.wait_timeout(q, Duration::from_millis(2)).unwrap().0;
            if q.samples.len() < ch && q.ended {
                for slot in &mut data[out_idx..] {
                    *slot = 0.0;
                }
                return;
            }
            if q.samples.len() < ch {
                for slot in &mut data[out_idx..] {
                    *slot = 0.0;
                }
                return;
            }
        }
        for c in 0..ch {
            data[out_idx + c] = q.samples.pop_front().unwrap_or(0.0);
        }
        out_idx += ch;
    }
    attack_meter_levels(data, channels, level_left, level_right);
}

fn stop_decode_locked(g: &mut PlaybackState) {
    g.decode_generation = g.decode_generation.wrapping_add(1);
    if let Ok(mut q) = g.queue.lock() {
        q.samples.clear();
        q.ended = true;
        q.generation = g.decode_generation;
    }
    g.tick_stop.store(true, Ordering::SeqCst);
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

fn spawn_decode_thread(
    path: String,
    start_sample: u64,
    sample_rate: u32,
    channels: u16,
    queue: Arc<Mutex<SampleQueue>>,
    queue_cv: Arc<Condvar>,
    generation: u64,
) {
    thread::spawn(move || {
        if let Err(e) = run_decode(
            &path,
            start_sample,
            sample_rate,
            channels,
            &queue,
            &queue_cv,
            generation,
        ) {
            eprintln!("playback decode error ({path}): {e}");
            if let Ok(mut q) = queue.lock() {
                if q.generation == generation {
                    q.ended = true;
                }
            }
            queue_cv.notify_all();
        }
    });
}

fn run_decode(
    path: &str,
    start_sample: u64,
    out_rate: u32,
    out_channels: u16,
    queue: &Arc<Mutex<SampleQueue>>,
    queue_cv: &Arc<Condvar>,
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

    let mut samples_out: u64 = 0;
    let target_start = start_sample;

    loop {
        if queue.lock().map_err(|e| e.to_string())?.generation != generation {
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
            let mut q = queue.lock().map_err(|e| e.to_string())?;
            if q.generation != generation {
                return Ok(());
            }
            for &s in chunk {
                q.samples.push_back(s);
            }
            drop(q);
            queue_cv.notify_all();
        }
    }

    if let Ok(mut q) = queue.lock() {
        if q.generation == generation {
            q.ended = true;
        }
    }
    queue_cv.notify_all();
    Ok(())
}

fn audio_buffer_to_interleaved(buf: &AudioBufferRef<'_>) -> Vec<f32> {
    match buf {
        AudioBufferRef::F32(b) => {
            let channels = b.spec().channels.count();
            let frames = b.frames();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for c in 0..channels {
                    out.push(b.chan(c)[f]);
                }
            }
            out
        }
        AudioBufferRef::S16(b) => {
            let channels = b.spec().channels.count();
            let frames = b.frames();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for c in 0..channels {
                    out.push(b.chan(c)[f] as f32 / i16::MAX as f32);
                }
            }
            out
        }
        AudioBufferRef::S32(b) => {
            let channels = b.spec().channels.count();
            let frames = b.frames();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for c in 0..channels {
                    out.push(b.chan(c)[f] as f32 / i32::MAX as f32);
                }
            }
            out
        }
        _ => Vec::new(),
    }
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

fn spawn_tick_thread(
    app: AppHandle,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    duration_ms: u64,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
) {
    thread::spawn(move || {
        let mut last = std::time::Instant::now();
        while !tick_stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
            if playing.load(Ordering::SeqCst) {
                let elapsed = last.elapsed().as_millis() as u64;
                last = std::time::Instant::now();
                let next = (position_ms.load(Ordering::SeqCst) + elapsed).min(duration_ms);
                position_ms.store(next, Ordering::SeqCst);
                if next >= duration_ms {
                    playing.store(false, Ordering::SeqCst);
                }
                release_meter_levels(&level_left, &level_right, 0.78);
            } else {
                last = std::time::Instant::now();
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
pub fn playback_stop(engine: tauri::State<'_, Arc<PlaybackEngine>>) -> Result<(), String> {
    engine.stop()
}
