//! Desktop audio preview playback (CoreAudio HAL via cpal on macOS).
//!
//! Architecture: symphonia decodes on a worker thread and pushes whole frames into a `ringbuf`
//! producer; the output callback only pops from the consumer, so it takes no locks, makes no
//! syscalls and never allocates. Control threads and the callback talk through generation
//! counters:
//!
//! - `decode_generation` identifies the current decode job. It is bumped under the producer
//!   lock, so a superseded decode thread can never push another sample after the bump.
//! - `stream_generation` is bumped after that; the callback flushes the ring when it sees a new
//!   value and echoes it in `ack_generation`. A decode thread waits for that echo before its
//!   first push, so nothing it pushes can be flushed and no stale sample survives a seek.
//!
//! Output is behind [`OutputBackend`] and events behind [`PlaybackEvents`], so the engine runs
//! without a device or a Tauri app in tests.

use std::any::Any;
use std::fs::File;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};
use serde::Serialize;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tauri::{AppHandle, Emitter};

use crate::tempo::{TempoEngine, TempoParams, CHUNK_FRAMES, MAX_DEVICE_CHANNELS};
use fourfour_stretch::Preset;

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_BUFFER_FRAMES: u32 = 128;
/// ~2s stereo @ 48 kHz — back-pressure on decode thread if the ring fills.
const RING_SAMPLES: usize = 48_000 * 2 * 2;
/// Crossfade length applied when new audio follows a stream switch (play/seek/stop) or a
/// dropout. Ramps from the last sample played into the new position so the waveform
/// discontinuity doesn't produce an audible click/pop. The ramp starts when the new audio
/// arrives, not at the switch: a slow decoder must not use it up while the ring is empty.
/// ~5ms at 48 kHz.
const DECLICK_FRAMES: usize = 256;
/// Per-frame decay applied to the last sample while the ring is empty (underrun), so running dry
/// fades to silence in about 2 ms instead of cutting off.
const UNDERRUN_DECAY: f32 = 0.98;
/// Max channels tracked for the declick crossfade seam.
const MAX_CH: usize = 8;
const TICK_INTERVAL: Duration = Duration::from_millis(50);
const MAX_CONSECUTIVE_RESETS: u32 = 8;
const STRETCH_PRESET: Preset = Preset::REALTIME;

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
    /// Tempo ratio the callback is playing now (1.0 = the track's own tempo).
    pub tempo_ratio: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaybackError {
    pub message: String,
    pub path: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct AudioOutputDevice {
    pub name: String,
    pub is_default: bool,
}

pub trait PlaybackEvents: Send + Sync + 'static {
    fn tick(&self, tick: PlaybackTick);
    fn error(&self, error: PlaybackError);
}

impl PlaybackEvents for AppHandle {
    fn tick(&self, tick: PlaybackTick) {
        let _ = self.emit("playback-tick", tick);
    }

    fn error(&self, error: PlaybackError) {
        let _ = self.emit("playback-error", error);
    }
}

pub type RenderFn = Box<dyn FnMut(&mut [f32], u16) + Send + 'static>;
pub type OutputErrorFn = Box<dyn Fn(String) + Send + Sync + 'static>;

pub struct OutputRequest {
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub buffer_frames: u32,
}

pub struct OpenedOutput {
    pub channels: u16,
    pub sample_rate: u32,
    /// Dropping this closes the output stream.
    pub stream: Box<dyn Any + Send>,
}

pub trait OutputBackend: Send + Sync + 'static {
    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, String>;
    /// `render` is called from the device's real-time thread with an interleaved f32 buffer and
    /// the channel count. `on_error` reports device failures (unplugged, format change).
    fn open(
        &self,
        request: &OutputRequest,
        render: RenderFn,
        on_error: OutputErrorFn,
    ) -> Result<OpenedOutput, String>;
}

struct EventHub {
    events: Mutex<Option<Arc<dyn PlaybackEvents>>>,
}

impl EventHub {
    fn set(&self, events: Arc<dyn PlaybackEvents>) {
        *lock_ignoring_poison(&self.events) = Some(events);
    }

    fn error(&self, message: String, path: Option<String>) {
        eprintln!("playback error: {message}");
        let events = lock_ignoring_poison(&self.events).clone();
        if let Some(events) = events {
            events.error(PlaybackError { message, path });
        }
    }
}

fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

struct ThreadGuard(Arc<AtomicUsize>);

impl ThreadGuard {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter.clone())
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct SamplePipe {
    producer: Arc<Mutex<HeapProd<f32>>>,
    /// Bumped on each play/seek/stop so the callback flushes stale samples.
    stream_generation: Arc<AtomicU64>,
    /// Echo of `stream_generation` written by the callback after it flushed the ring.
    ack_generation: Arc<AtomicU64>,
    decode_generation: Arc<AtomicU64>,
    /// Decode generation whose decode job pushed its last sample (finished or failed).
    ended_generation: Arc<AtomicU64>,
    /// Decode generation for which the callback saw the ring run dry after `ended_generation`.
    drained_generation: Arc<AtomicU64>,
    /// Frames the callback consumed since it last flushed. Reset by the callback; read together
    /// with `ack_generation` so a tick never combines a new start position with an old count.
    frames_played: Arc<AtomicU64>,
}

fn new_sample_pipe() -> (SamplePipe, HeapCons<f32>) {
    let rb = HeapRb::<f32>::new(RING_SAMPLES);
    let (prod, cons) = rb.split();
    let pipe = SamplePipe {
        producer: Arc::new(Mutex::new(prod)),
        stream_generation: Arc::new(AtomicU64::new(0)),
        ack_generation: Arc::new(AtomicU64::new(0)),
        decode_generation: Arc::new(AtomicU64::new(0)),
        ended_generation: Arc::new(AtomicU64::new(0)),
        drained_generation: Arc::new(AtomicU64::new(0)),
        frames_played: Arc::new(AtomicU64::new(0)),
    };
    (pipe, cons)
}

impl SamplePipe {
    /// Invalidate the current decode job and flush the ring. Taking the producer lock makes the
    /// bump atomic with respect to a decode thread's push, so after this returns the old thread
    /// cannot add anything.
    fn supersede(&self) -> u64 {
        let generation = {
            let _producer = lock_ignoring_poison(&self.producer);
            self.decode_generation.fetch_add(1, Ordering::SeqCst) + 1
        };
        self.stream_generation.fetch_add(1, Ordering::SeqCst);
        generation
    }
}

/// Everything the output callback owns. No locks, no allocation in `render`.
struct RenderState {
    cons: HeapCons<f32>,
    pipe: SamplePipe,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
    tempo: Arc<TempoParams>,
    engine: TempoEngine,
    chunk: Vec<f32>,
    callback_generation: u64,
    declick_remaining: usize,
    starved: bool,
    seam: [f32; MAX_CH],
    last_sample: [f32; MAX_CH],
}

impl RenderState {
    fn new(
        cons: HeapCons<f32>,
        pipe: SamplePipe,
        level_left: Arc<AtomicU32>,
        level_right: Arc<AtomicU32>,
        tempo: Arc<TempoParams>,
        sample_rate: u32,
        preset: Preset,
    ) -> Self {
        Self {
            cons,
            pipe,
            level_left,
            level_right,
            tempo,
            engine: TempoEngine::new(sample_rate, preset),
            chunk: vec![0.0; CHUNK_FRAMES * MAX_DEVICE_CHANNELS],
            callback_generation: 0,
            declick_remaining: 0,
            starved: false,
            seam: [0.0; MAX_CH],
            last_sample: [0.0; MAX_CH],
        }
    }

    fn render(&mut self, data: &mut [f32], channels: u16) {
        let stream_gen = self.pipe.stream_generation.load(Ordering::Acquire);
        if stream_gen != self.callback_generation {
            self.cons.clear();
            self.engine.flush();
            self.pipe.frames_played.store(0, Ordering::Relaxed);
            self.callback_generation = stream_gen;
            self.declick_remaining = 0;
            self.starved = true;
            self.pipe.ack_generation.store(stream_gen, Ordering::Release);
        }

        let ch = (channels.max(1) as usize).min(MAX_DEVICE_CHANNELS);
        let ramp = DECLICK_FRAMES as f32;
        let decode_gen = self.pipe.decode_generation.load(Ordering::Acquire);
        let ended = self.pipe.ended_generation.load(Ordering::Acquire) == decode_gen;
        let target = self.tempo.target();
        let chunk_frames = CHUNK_FRAMES.min(CHUNK_FRAMES * MAX_DEVICE_CHANNELS / ch);

        for block in data.chunks_mut(chunk_frames * ch) {
            let frames = block.len() / ch;
            let result = self.engine.process_chunk(
                self.cons.as_slices(),
                ch,
                ended,
                target,
                &mut self.chunk[..frames * ch],
            );
            self.cons.skip(result.popped_frames * ch);

            for (index, frame) in block.chunks_mut(ch).enumerate() {
                let has_data = index < result.valid_frames;
                if has_data && self.starved {
                    self.starved = false;
                    self.seam = self.last_sample;
                    self.declick_remaining = DECLICK_FRAMES;
                }
                let (mix_old, mix_new) = if self.declick_remaining > 0 {
                    let t = 1.0 - (self.declick_remaining as f32 / ramp);
                    (1.0 - t, t)
                } else {
                    (0.0, 1.0)
                };
                for (c, slot) in frame.iter_mut().enumerate() {
                    let seam_v = if c < MAX_CH { self.seam[c] } else { 0.0 };
                    let out = if has_data {
                        self.chunk[index * ch + c] * mix_new + seam_v * mix_old
                    } else if self.declick_remaining > 0 {
                        seam_v * mix_old
                    } else {
                        self.starved = true;
                        if c < MAX_CH {
                            self.last_sample[c] * UNDERRUN_DECAY
                        } else {
                            0.0
                        }
                    };
                    *slot = out;
                    if c < MAX_CH {
                        self.last_sample[c] = out;
                    }
                }
                if self.declick_remaining > 0 {
                    self.declick_remaining -= 1;
                }
            }
        }

        self.pipe
            .frames_played
            .store(self.engine.heard_frames(), Ordering::Relaxed);
        self.tempo.publish_live_ratio(self.engine.rate());
        if self.pipe.ended_generation.load(Ordering::Acquire) == decode_gen && self.cons.is_empty()
        {
            self.pipe
                .drained_generation
                .store(decode_gen, Ordering::Release);
        }
        attack_meter_levels(data, channels, &self.level_left, &self.level_right);
    }
}

struct OutputInfo {
    channels: u16,
    sample_rate: u32,
    epoch: u64,
}

struct Playhead {
    start_ms: u64,
    ack_target: u64,
}

struct Inner {
    config: PlaybackAudioConfig,
    device_name: Option<String>,
    output: Option<OutputInfo>,
    output_epoch: Arc<AtomicU64>,
    stream: Option<Box<dyn Any + Send>>,
    path: Option<String>,
    duration_ms: u64,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    pipe: SamplePipe,
    playhead: Playhead,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
    tempo: Arc<TempoParams>,
}

pub struct PlaybackEngine {
    backend: Arc<dyn OutputBackend>,
    inner: Mutex<Inner>,
    hub: Arc<EventHub>,
    device_lost: Arc<AtomicBool>,
    live_threads: Arc<AtomicUsize>,
}

impl PlaybackEngine {
    /// Never fails for a missing device: the engine opens the output lazily on the first play,
    /// so commands keep returning errors instead of an unmanaged-state panic.
    pub fn new() -> Result<Self, String> {
        Ok(Self::with_backend(Arc::new(CpalBackend)))
    }

    pub fn with_backend(backend: Arc<dyn OutputBackend>) -> Self {
        let (pipe, _cons) = new_sample_pipe();
        let engine = Self {
            backend,
            inner: Mutex::new(Inner {
                config: PlaybackAudioConfig::default(),
                device_name: None,
                output: None,
                output_epoch: Arc::new(AtomicU64::new(0)),
                stream: None,
                path: None,
                duration_ms: 0,
                position_ms: Arc::new(AtomicU64::new(0)),
                playing: Arc::new(AtomicBool::new(false)),
                tick_stop: Arc::new(AtomicBool::new(true)),
                pipe,
                playhead: Playhead {
                    start_ms: 0,
                    ack_target: 0,
                },
                level_left: Arc::new(AtomicU32::new(0)),
                level_right: Arc::new(AtomicU32::new(0)),
                tempo: Arc::new(TempoParams::new()),
            }),
            hub: Arc::new(EventHub {
                events: Mutex::new(None),
            }),
            device_lost: Arc::new(AtomicBool::new(false)),
            live_threads: Arc::new(AtomicUsize::new(0)),
        };
        {
            let mut inner = engine.lock();
            if let Err(e) = engine.open_output(&mut inner) {
                eprintln!("audio output unavailable at startup, will retry on play: {e}");
            }
        }
        engine
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        lock_ignoring_poison(&self.inner)
    }

    pub fn live_thread_count(&self) -> usize {
        self.live_threads.load(Ordering::SeqCst)
    }

    pub fn list_output_devices(&self) -> Result<Vec<AudioOutputDevice>, String> {
        self.backend.list_devices()
    }

    pub fn output_device_name(&self) -> Result<Option<String>, String> {
        Ok(self.lock().device_name.clone())
    }

    pub fn set_output_device(&self, name: Option<String>) -> Result<(), String> {
        let mut inner = self.lock();
        self.stop_locked(&mut inner);
        inner.device_name = name;
        self.open_output(&mut inner)
    }

    pub fn configure(
        &self,
        sample_rate: u32,
        buffer_frames: u32,
    ) -> Result<PlaybackAudioConfig, String> {
        let config = PlaybackAudioConfig {
            sample_rate: sample_rate.clamp(8_000, 192_000),
            buffer_frames: buffer_frames.clamp(32, 4096),
        };
        let mut inner = self.lock();
        self.stop_locked(&mut inner);
        inner.config = config;
        self.open_output(&mut inner)?;
        Ok(config)
    }

    pub fn audio_config(&self) -> Result<PlaybackAudioConfig, String> {
        Ok(self.lock().config)
    }

    /// Drops the old stream first (a device may not allow two streams), then opens a new one.
    /// On failure `output` stays `None` and the next play retries.
    fn open_output(&self, inner: &mut Inner) -> Result<(), String> {
        inner.stream = None;
        inner.output = None;
        let epoch = inner.output_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let (pipe, cons) = new_sample_pipe();
        let mut render_state = RenderState::new(
            cons,
            pipe.clone(),
            inner.level_left.clone(),
            inner.level_right.clone(),
            inner.tempo.clone(),
            inner.config.sample_rate,
            STRETCH_PRESET,
        );
        let active_epoch = inner.output_epoch.clone();
        let render: RenderFn = Box::new(move |data, channels| {
            if active_epoch.load(Ordering::SeqCst) != epoch {
                data.fill(0.0);
                return;
            }
            render_state.render(data, channels);
        });
        let hub = self.hub.clone();
        let device_lost = self.device_lost.clone();
        let on_error: OutputErrorFn = Box::new(move |message| {
            device_lost.store(true, Ordering::SeqCst);
            hub.error(format!("Audio output error: {message}"), None);
        });
        let request = OutputRequest {
            device_name: inner.device_name.clone(),
            sample_rate: inner.config.sample_rate,
            buffer_frames: inner.config.buffer_frames,
        };
        let opened = self.backend.open(&request, render, on_error)?;
        self.device_lost.store(false, Ordering::SeqCst);
        inner.pipe = pipe;
        inner.stream = Some(opened.stream);
        inner.output = Some(OutputInfo {
            channels: opened.channels,
            sample_rate: opened.sample_rate,
            epoch,
        });
        Ok(())
    }

    fn ensure_output(&self, inner: &mut Inner) -> Result<(u32, u16, u64), String> {
        if inner.output.is_none() || self.device_lost.load(Ordering::SeqCst) {
            self.stop_locked(inner);
            self.open_output(inner)?;
        }
        let out = inner
            .output
            .as_ref()
            .ok_or_else(|| "Audio output not initialized".to_string())?;
        Ok((out.sample_rate, out.channels, out.epoch))
    }

    pub fn play(
        &self,
        events: Arc<dyn PlaybackEvents>,
        path: &str,
        position_ms: u64,
        duration_ms_hint: Option<u64>,
    ) -> Result<(), String> {
        self.hub.set(events.clone());
        std::fs::metadata(path).map_err(|e| format!("Open {path}: {e}"))?;

        let (_, _, epoch) = {
            let mut inner = self.lock();
            self.ensure_output(&mut inner)?
        };

        let duration_ms = match duration_ms_hint.filter(|d| *d > 0) {
            Some(duration_ms) => duration_ms,
            None => probe_duration_ms(path)?,
        };

        let mut inner = self.lock();
        let still_current = inner.output.as_ref().is_some_and(|o| o.epoch == epoch);
        if !still_current {
            return Err("Audio output changed while loading; try again".to_string());
        }
        self.start_locked(&mut inner, events, path.to_string(), position_ms, duration_ms)
    }

    fn start_locked(
        &self,
        inner: &mut Inner,
        events: Arc<dyn PlaybackEvents>,
        path: String,
        position_ms: u64,
        duration_ms: u64,
    ) -> Result<(), String> {
        let (sample_rate, channels, _) = self.ensure_output(inner)?;
        inner.tick_stop.store(true, Ordering::SeqCst);
        let generation = inner.pipe.supersede();
        let ack_target = inner.pipe.stream_generation.load(Ordering::SeqCst);

        let start_position_ms = if duration_ms > 0 {
            position_ms.min(duration_ms)
        } else {
            position_ms
        };
        inner.path = Some(path.clone());
        inner.duration_ms = duration_ms;
        inner.position_ms.store(start_position_ms, Ordering::SeqCst);
        inner.playing.store(true, Ordering::SeqCst);
        inner.playhead = Playhead {
            start_ms: start_position_ms,
            ack_target,
        };

        // A fresh stop flag per tick thread: the previous one was just set, and sharing a flag
        // would let a new play reset it before the old thread woke, leaving zombie tickers.
        let tick_stop = Arc::new(AtomicBool::new(false));
        inner.tick_stop = tick_stop.clone();

        let start_sample = (start_position_ms as u128)
            .saturating_mul(sample_rate as u128)
            .saturating_mul(channels.max(1) as u128)
            / 1000;

        spawn_decode_thread(DecodeJob {
            path,
            start_sample: start_sample as u64,
            sample_rate,
            channels,
            pipe: inner.pipe.clone(),
            generation,
            ack_target,
            hub: self.hub.clone(),
            guard: ThreadGuard::new(&self.live_threads),
        });
        spawn_tick_thread(TickContext {
            events,
            position_ms: inner.position_ms.clone(),
            playing: inner.playing.clone(),
            tick_stop,
            pipe: inner.pipe.clone(),
            level_left: inner.level_left.clone(),
            level_right: inner.level_right.clone(),
            tempo: inner.tempo.clone(),
            device_lost: self.device_lost.clone(),
            duration_ms,
            start_ms: start_position_ms,
            ack_target,
            generation,
            sample_rate,
            guard: ThreadGuard::new(&self.live_threads),
        });
        Ok(())
    }

    fn clock_position_ms(&self, inner: &Inner) -> u64 {
        let rate = inner.output.as_ref().map_or(1, |o| o.sample_rate as u64);
        clock_position_ms(
            &inner.pipe,
            inner.playhead.start_ms,
            inner.playhead.ack_target,
            rate,
            inner.duration_ms,
        )
    }

    pub fn pause(&self) -> Result<(), String> {
        let inner = self.lock();
        if inner.playing.load(Ordering::SeqCst) {
            inner
                .position_ms
                .store(self.clock_position_ms(&inner), Ordering::SeqCst);
        }
        inner.playing.store(false, Ordering::SeqCst);
        inner.pipe.supersede();
        Ok(())
    }

    pub fn resume(&self, events: Arc<dyn PlaybackEvents>) -> Result<(), String> {
        self.hub.set(events.clone());
        let mut inner = self.lock();
        let Some(path) = inner.path.clone() else {
            return Ok(());
        };
        let position_ms = inner.position_ms.load(Ordering::SeqCst);
        let duration_ms = inner.duration_ms;
        if position_ms >= duration_ms && duration_ms > 0 {
            return Ok(());
        }
        self.start_locked(&mut inner, events, path, position_ms, duration_ms)
    }

    pub fn set_position(&self, position_ms: u64) -> Result<(), String> {
        let inner = self.lock();
        let clamped = if inner.duration_ms > 0 {
            position_ms.min(inner.duration_ms)
        } else {
            position_ms
        };
        inner.position_ms.store(clamped, Ordering::SeqCst);
        Ok(())
    }

    pub fn seek(
        &self,
        events: Arc<dyn PlaybackEvents>,
        position_ms: u64,
    ) -> Result<(), String> {
        self.hub.set(events.clone());
        let mut inner = self.lock();
        let Some(path) = inner.path.clone() else {
            return Ok(());
        };
        // Reuse the duration established at play() so the playhead/beat-grid timebase stays
        // stable across scrubs.
        let duration_ms = inner.duration_ms;
        self.start_locked(&mut inner, events, path, position_ms, duration_ms)
    }

    /// Unloading a track also drops its tempo and key shift, like ejecting on a CDJ.
    pub fn stop(&self) -> Result<(), String> {
        let mut inner = self.lock();
        self.stop_locked(&mut inner);
        inner.tempo.reset();
        Ok(())
    }

    pub fn set_tempo(&self, ratio: f64) {
        self.lock().tempo.set_ratio(ratio);
    }

    pub fn set_key_shift(&self, semitones: f64) {
        self.lock().tempo.set_semitones(semitones);
    }

    pub fn set_key_lock(&self, on: bool) {
        self.lock().tempo.set_key_lock(on);
    }


    fn stop_locked(&self, inner: &mut Inner) {
        inner.tick_stop.store(true, Ordering::SeqCst);
        inner.pipe.supersede();
        inner.path = None;
        inner.duration_ms = 0;
        inner.position_ms.store(0, Ordering::SeqCst);
        inner.playing.store(false, Ordering::SeqCst);
        inner.level_left.store(0, Ordering::Relaxed);
        inner.level_right.store(0, Ordering::Relaxed);
    }

    /// Stops playback and waits for the decode and tick threads to exit.
    pub fn shutdown(&self, timeout: Duration) -> bool {
        {
            let mut inner = self.lock();
            self.stop_locked(&mut inner);
            inner.stream = None;
            inner.output = None;
        }
        let deadline = Instant::now() + timeout;
        while self.live_thread_count() > 0 {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(2));
        }
        true
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.shutdown(Duration::from_secs(2));
    }
}

fn clock_position_ms(
    pipe: &SamplePipe,
    start_ms: u64,
    ack_target: u64,
    sample_rate: u64,
    duration_ms: u64,
) -> u64 {
    let position = if pipe.ack_generation.load(Ordering::Acquire) == ack_target {
        start_ms + pipe.frames_played.load(Ordering::Relaxed) * 1000 / sample_rate.max(1)
    } else {
        start_ms
    };
    if duration_ms > 0 {
        position.min(duration_ms)
    } else {
        position
    }
}

struct CpalBackend;

/// cpal streams are `!Send` on some hosts; the engine only keeps the handle alive and drops it.
struct AudioStreamHandle(#[allow(dead_code)] cpal::Stream);
unsafe impl Send for AudioStreamHandle {}

impl OutputBackend for CpalBackend {
    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, String> {
        list_output_devices()
    }

    fn open(
        &self,
        request: &OutputRequest,
        mut render: RenderFn,
        on_error: OutputErrorFn,
    ) -> Result<OpenedOutput, String> {
        let device = resolve_output_device(request.device_name.as_deref())?;
        let (supported, stream_config) =
            resolve_stream_config(&device, request.sample_rate, request.buffer_frames)?;
        let channels = stream_config.channels;
        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| render(data, channels),
                move |err| on_error(err.to_string()),
                None,
            )
            .map_err(|e| format!("Failed to open CoreAudio output stream: {e}"))?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(OpenedOutput {
            channels,
            sample_rate: supported.sample_rate().0,
            stream: Box::new(AudioStreamHandle(stream)),
        })
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

    let default = device.default_output_config().map_err(|e| e.to_string())?;
    if default.sample_format() != SampleFormat::F32 {
        return Err("Default output device does not support f32 samples".to_string());
    }
    let stream_config = stream_config_with_buffer(&default, buffer_frames);
    Ok((default, stream_config))
}

fn stream_config_with_buffer(
    supported: &SupportedStreamConfig,
    buffer_frames: u32,
) -> StreamConfig {
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
        pk_r = match frame.get(1) {
            Some(r) => pk_r.max(r.abs()),
            None => pk_l,
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

fn list_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();
    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;
    for device in devices {
        let name = device
            .name()
            .unwrap_or_else(|_| "Unknown device".to_string());
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
        eprintln!(
            "Warning: Configured output device '{}' not found. Falling back to system default.",
            name
        );
    }
    host.default_output_device()
        .ok_or_else(|| "No default output audio device".to_string())
}

fn probe_format(
    path: &str,
) -> Result<Box<dyn symphonia::core::formats::FormatReader>, String> {
    let file = File::open(path).map_err(|e| format!("Open {path}: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported or corrupt audio file {path}: {e}"))?;
    Ok(probed.format)
}

fn probe_duration_ms(path: &str) -> Result<u64, String> {
    catch_unwind(AssertUnwindSafe(|| probe_duration_ms_unguarded(path)))
        .unwrap_or_else(|_| Err(format!("Decoder crashed probing {path}")))
}

fn probe_duration_ms_unguarded(path: &str) -> Result<u64, String> {
    let format = probe_format(path)?;
    let track = format
        .default_track()
        .ok_or_else(|| "No default audio track".to_string())?;
    let tb = track
        .codec_params
        .time_base
        .or(track
            .codec_params
            .sample_rate
            .map(|sr| symphonia::core::units::TimeBase {
                numer: 1,
                denom: sr,
            }))
        .ok_or_else(|| "Unknown track duration".to_string())?;
    let duration_frames = track.codec_params.n_frames.unwrap_or(0);
    if duration_frames == 0 {
        return Ok(0);
    }
    let time = tb.calc_time(duration_frames);
    Ok(time.seconds * 1000 + (time.frac * 1000.0) as u64)
}

/// Pushes whole frames. Returns `Ok(false)` once the job is superseded.
fn push_frames(
    pipe: &SamplePipe,
    generation: u64,
    ack_target: u64,
    channels: usize,
    mut samples: &[f32],
) -> Result<bool, String> {
    let ch = channels.max(1);
    while !samples.is_empty() {
        if pipe.decode_generation.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        if pipe.ack_generation.load(Ordering::Acquire) != ack_target {
            thread::sleep(Duration::from_micros(200));
            continue;
        }
        let pushed = {
            let mut producer = lock_ignoring_poison(&pipe.producer);
            if pipe.decode_generation.load(Ordering::SeqCst) != generation {
                return Ok(false);
            }
            let free = producer.vacant_len() / ch * ch;
            let n = free.min(samples.len());
            if n > 0 {
                producer.push_slice(&samples[..n]);
            }
            n
        };
        samples = &samples[pushed..];
        if !samples.is_empty() {
            thread::sleep(Duration::from_micros(500));
        }
    }
    Ok(true)
}

struct DecodeJob {
    path: String,
    start_sample: u64,
    sample_rate: u32,
    channels: u16,
    pipe: SamplePipe,
    generation: u64,
    ack_target: u64,
    hub: Arc<EventHub>,
    guard: ThreadGuard,
}

fn spawn_decode_thread(job: DecodeJob) {
    thread::spawn(move || {
        let DecodeJob {
            path,
            start_sample,
            sample_rate,
            channels,
            pipe,
            generation,
            ack_target,
            hub,
            guard: _guard,
        } = job;
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_decode(
                &path,
                start_sample,
                sample_rate,
                channels,
                &|| pipe.decode_generation.load(Ordering::SeqCst) == generation,
                &mut |samples| push_frames(&pipe, generation, ack_target, channels as usize, samples),
            )
        }));
        let failure = match result {
            Ok(Ok(DecodeOutcome::Superseded)) => return,
            Ok(Ok(DecodeOutcome::Finished)) => None,
            Ok(Err(e)) => Some(e),
            Err(_) => Some(format!("Decoder crashed on {path}")),
        };
        if pipe.decode_generation.load(Ordering::SeqCst) == generation {
            if let Some(message) = failure {
                hub.error(message, Some(path));
            }
            pipe.ended_generation.store(generation, Ordering::Release);
        }
    });
}

#[derive(Debug, PartialEq, Eq)]
enum DecodeOutcome {
    Finished,
    Superseded,
}

fn run_decode(
    path: &str,
    start_sample: u64,
    out_rate: u32,
    out_channels: u16,
    is_current: &dyn Fn() -> bool,
    sink: &mut dyn FnMut(&[f32]) -> Result<bool, String>,
) -> Result<DecodeOutcome, String> {
    let out_ch = out_channels.max(1) as usize;
    let mut format = probe_format(path)?;
    if !is_current() {
        return Ok(DecodeOutcome::Superseded);
    }
    let track = format
        .default_track()
        .ok_or_else(|| "No default audio track".to_string())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Unsupported codec in {path}: {e}"))?;
    let params_rate = track.codec_params.sample_rate.unwrap_or(out_rate);
    let time_base = track.codec_params.time_base;

    let target_start = start_sample;
    let mut samples_out: u64 = 0;

    // Seek the demuxer near the target instead of decoding-and-discarding every sample before it.
    if target_start > 0 {
        let target_seconds = start_sample as f64 / (out_rate as f64 * out_ch as f64);
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
                    None => seeked.actual_ts as f64 / params_rate as f64,
                };
                samples_out = (actual_seconds * out_rate as f64).round() as u64 * out_ch as u64;
            }
            // Seek unsupported for this source — fall back to decode-and-skip.
            Err(_) => samples_out = 0,
        }
    }

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut resampler: Option<Resampler> = None;
    let mut resampled: Vec<f32> = Vec::new();
    let mut mapped: Vec<f32> = Vec::new();
    let mut consecutive_resets = 0u32;

    loop {
        if !is_current() {
            return Ok(DecodeOutcome::Superseded);
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::ResetRequired) => {
                consecutive_resets += 1;
                if consecutive_resets > MAX_CONSECUTIVE_RESETS {
                    return Err(format!("Stream reset loop in {path}"));
                }
                decoder.reset();
                continue;
            }
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => return Err(format!("Read error in {path}: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(e)) => {
                eprintln!("playback skipped undecodable packet ({path}): {e}");
                continue;
            }
            Err(e) => return Err(format!("Decode error in {path}: {e}")),
        };
        consecutive_resets = 0;
        let spec = *decoded.spec();
        let in_rate = spec.rate;
        let in_channels = spec.channels.count().max(1);
        if decoded.frames() == 0 {
            continue;
        }
        let needed = decoded.capacity() * in_channels;
        if sample_buf.as_ref().is_none_or(|b| b.capacity() < needed) {
            sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }
        let buf = sample_buf
            .as_mut()
            .ok_or_else(|| "decode buffer missing".to_string())?;
        buf.copy_interleaved_ref(decoded);
        let mut frame_samples: &[f32] = buf.samples();

        if in_rate != out_rate {
            if !resampler
                .as_ref()
                .is_some_and(|r| r.matches(in_rate, in_channels))
            {
                resampler = Some(Resampler::new(in_rate, out_rate, in_channels));
            }
            let rs = resampler
                .as_mut()
                .ok_or_else(|| "resampler missing".to_string())?;
            resampled.clear();
            rs.process(frame_samples, &mut resampled);
            frame_samples = &resampled;
        }
        let out_samples: &[f32] = if in_channels == out_ch {
            frame_samples
        } else {
            map_channels(frame_samples, in_channels, out_ch, &mut mapped);
            &mapped
        };

        let len = out_samples.len() as u64;
        if samples_out + len <= target_start {
            samples_out += len;
            continue;
        }
        let skip = (target_start.saturating_sub(samples_out) as usize) / out_ch * out_ch;
        samples_out += len;
        if !sink(&out_samples[skip..])? {
            return Ok(DecodeOutcome::Superseded);
        }
    }

    Ok(DecodeOutcome::Finished)
}

/// Streaming linear resampler. Keeps its read phase and the last frame across packets so packet
/// boundaries add no rounding drift or discontinuity.
struct Resampler {
    in_rate: u32,
    channels: usize,
    step: f64,
    pos: f64,
    prev: Vec<f32>,
    primed: bool,
}

impl Resampler {
    fn new(in_rate: u32, out_rate: u32, channels: usize) -> Self {
        Self {
            in_rate,
            channels,
            step: in_rate as f64 / out_rate.max(1) as f64,
            pos: 0.0,
            prev: vec![0.0; channels],
            primed: false,
        }
    }

    fn matches(&self, in_rate: u32, channels: usize) -> bool {
        self.in_rate == in_rate && self.channels == channels
    }

    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let ch = self.channels;
        let frames = input.len() / ch;
        if frames == 0 {
            return;
        }
        if !self.primed {
            self.prev.copy_from_slice(&input[..ch]);
            self.pos = 1.0;
            self.primed = true;
        }
        let frames_f = frames as f64;
        out.reserve((frames as f64 / self.step) as usize * ch + ch);
        while self.pos < frames_f {
            let i = self.pos as usize;
            let frac = (self.pos - i as f64) as f32;
            for c in 0..ch {
                let a = if i == 0 {
                    self.prev[c]
                } else {
                    input[(i - 1) * ch + c]
                };
                let b = input[i * ch + c];
                out.push(a + (b - a) * frac);
            }
            self.pos += self.step;
        }
        self.pos -= frames_f;
        self.prev.copy_from_slice(&input[(frames - 1) * ch..frames * ch]);
    }
}

fn map_channels(samples: &[f32], in_channels: usize, out_channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if in_channels == 0 || out_channels == 0 {
        return;
    }
    let frames = samples.len() / in_channels;
    out.reserve(frames * out_channels);
    for f in 0..frames {
        let base = f * in_channels;
        let l = samples[base];
        let r = if in_channels > 1 { samples[base + 1] } else { l };
        match out_channels {
            1 => out.push((l + r) * 0.5),
            _ => {
                out.push(l);
                out.push(r);
                out.extend(std::iter::repeat_n(0.0, out_channels - 2));
            }
        }
    }
}

struct TickContext {
    events: Arc<dyn PlaybackEvents>,
    position_ms: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    tick_stop: Arc<AtomicBool>,
    pipe: SamplePipe,
    level_left: Arc<AtomicU32>,
    level_right: Arc<AtomicU32>,
    tempo: Arc<TempoParams>,
    device_lost: Arc<AtomicBool>,
    duration_ms: u64,
    start_ms: u64,
    ack_target: u64,
    generation: u64,
    sample_rate: u32,
    guard: ThreadGuard,
}

fn spawn_tick_thread(ctx: TickContext) {
    thread::spawn(move || {
        let _guard = &ctx.guard;
        let rate = ctx.sample_rate.max(1) as u64;
        while !ctx.tick_stop.load(Ordering::SeqCst) {
            thread::sleep(TICK_INTERVAL);
            if ctx.tick_stop.load(Ordering::SeqCst) {
                break;
            }
            if ctx.playing.load(Ordering::SeqCst) {
                let next = clock_position_ms(
                    &ctx.pipe,
                    ctx.start_ms,
                    ctx.ack_target,
                    rate,
                    ctx.duration_ms,
                );
                ctx.position_ms.store(next, Ordering::SeqCst);
                let reached_end = ctx.duration_ms > 0 && next >= ctx.duration_ms;
                let drained = ctx.pipe.drained_generation.load(Ordering::Acquire) == ctx.generation
                    && ctx.pipe.ack_generation.load(Ordering::Acquire) == ctx.ack_target;
                if reached_end || drained || ctx.device_lost.load(Ordering::SeqCst) {
                    ctx.playing.store(false, Ordering::SeqCst);
                }
                release_meter_levels(&ctx.level_left, &ctx.level_right, 0.78);
            } else {
                release_meter_levels(&ctx.level_left, &ctx.level_right, 0.55);
            }
            let playing = ctx.playing.load(Ordering::SeqCst);
            let level_left = load_f32(&ctx.level_left);
            let level_right = load_f32(&ctx.level_right);
            ctx.events.tick(PlaybackTick {
                position_ms: ctx.position_ms.load(Ordering::SeqCst),
                duration_ms: ctx.duration_ms,
                playing,
                level_left,
                level_right,
                tempo_ratio: ctx.tempo.live_ratio() as f32,
            });
            if !playing && level_left == 0.0 && level_right == 0.0 {
                break;
            }
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
    engine.play(Arc::new(app), &path, position_ms, duration_ms)
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
    engine.resume(Arc::new(app))
}

#[tauri::command]
pub fn playback_seek(
    position_ms: u64,
    app: AppHandle,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.seek(Arc::new(app), position_ms)
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
pub fn playback_set_tempo(
    ratio: f64,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.set_tempo(ratio);
    Ok(())
}

#[tauri::command]
pub fn playback_set_key_shift(
    semitones: f64,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.set_key_shift(semitones);
    Ok(())
}

#[tauri::command]
pub fn playback_set_key_lock(
    on: bool,
    engine: tauri::State<'_, Arc<PlaybackEngine>>,
) -> Result<(), String> {
    engine.set_key_lock(on);
    Ok(())
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

#[cfg(test)]
#[path = "playback_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tempo_tests.rs"]
mod tempo_tests;

#[cfg(test)]
#[path = "tempo_lab.rs"]
mod tempo_lab;
