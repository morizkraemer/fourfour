use super::*;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

struct CountingAlloc;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn count_if_armed() {
    if COUNTING.with(|c| c.get()) {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_if_armed();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count_if_armed();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_if_armed();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

pub(crate) fn write_wav(
    path: &Path,
    rate: u32,
    channels: u16,
    frames: u32,
    sample: impl Fn(u32, u16) -> f32,
) {
    let data_len = frames * channels as u32 * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..frames {
        for c in 0..channels {
            let v = (sample(n, c) * i16::MAX as f32) as i16;
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(path, bytes).unwrap();
}

fn sine_wav(path: &Path, rate: u32, channels: u16, seconds: u32, hz: f32) {
    write_wav(path, rate, channels, rate * seconds, |n, _| {
        f32::sin(2.0 * std::f32::consts::PI * hz * n as f32 / rate as f32) * 0.5
    });
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fourfour-playback-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

pub(crate) fn decode_to_vec(path: &Path, start_sample: u64, rate: u32, channels: u16) -> Vec<f32> {
    let mut out = Vec::new();
    let outcome = run_decode(
        path.to_str().unwrap(),
        start_sample,
        rate,
        channels,
        &|| true,
        &mut |samples| {
            out.extend_from_slice(samples);
            Ok(true)
        },
    )
    .unwrap();
    assert_eq!(outcome, DecodeOutcome::Finished);
    out
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, s| m.max(s.abs()))
}

struct FakeBackend {
    render: Mutex<Option<RenderFn>>,
    on_error: Mutex<Option<OutputErrorFn>>,
    channels: u16,
    forced_rate: Mutex<Option<u32>>,
    opens: AtomicUsize,
    fail_open: AtomicBool,
}

impl FakeBackend {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            render: Mutex::new(None),
            on_error: Mutex::new(None),
            channels: 2,
            forced_rate: Mutex::new(None),
            opens: AtomicUsize::new(0),
            fail_open: AtomicBool::new(false),
        })
    }

    fn render_into(&self, buf: &mut [f32]) {
        let mut render = self.render.lock().unwrap();
        match render.as_mut() {
            Some(render) => render(buf, self.channels),
            None => buf.fill(0.0),
        }
    }

    fn render_frames(&self, frames: usize) -> Vec<f32> {
        let mut buf = vec![0.0; frames * self.channels as usize];
        self.render_into(&mut buf);
        buf
    }

    /// Clock-driven device: one 128-frame buffer every `period`.
    fn pump(&self, buffers: usize, period: Duration) -> Vec<f32> {
        let mut out = Vec::with_capacity(buffers * 128 * self.channels as usize);
        for _ in 0..buffers {
            out.extend(self.render_frames(128));
            if !period.is_zero() {
                thread::sleep(period);
            }
        }
        out
    }

    fn pump_until(
        &self,
        timeout: Duration,
        mut done: impl FnMut(&[f32]) -> bool,
    ) -> Option<Instant> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let buf = self.render_frames(128);
            if done(&buf) {
                return Some(Instant::now());
            }
            thread::sleep(Duration::from_micros(300));
        }
        None
    }

    fn inject_error(&self, message: &str) {
        if let Some(f) = self.on_error.lock().unwrap().as_ref() {
            f(message.to_string());
        }
    }
}

impl OutputBackend for FakeBackend {
    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, String> {
        Ok(vec![AudioOutputDevice {
            name: "fake".into(),
            is_default: true,
        }])
    }

    fn open(
        &self,
        request: &OutputRequest,
        render: RenderFn,
        on_error: OutputErrorFn,
    ) -> Result<OpenedOutput, String> {
        if self.fail_open.load(Ordering::SeqCst) {
            return Err("fake device unavailable".into());
        }
        self.opens.fetch_add(1, Ordering::SeqCst);
        *self.render.lock().unwrap() = Some(render);
        *self.on_error.lock().unwrap() = Some(on_error);
        let rate = self.forced_rate.lock().unwrap().unwrap_or(request.sample_rate);
        Ok(OpenedOutput {
            channels: self.channels,
            sample_rate: rate,
            stream: Box::new(()),
        })
    }
}

#[derive(Default)]
struct RecordingEvents {
    ticks: Mutex<Vec<PlaybackTick>>,
    errors: Mutex<Vec<PlaybackError>>,
}

impl PlaybackEvents for RecordingEvents {
    fn tick(&self, tick: PlaybackTick) {
        self.ticks.lock().unwrap().push(tick);
    }

    fn error(&self, error: PlaybackError) {
        self.errors.lock().unwrap().push(error);
    }
}

impl RecordingEvents {
    fn tick_count(&self) -> usize {
        self.ticks.lock().unwrap().len()
    }

    fn last_playing(&self) -> Option<bool> {
        self.ticks.lock().unwrap().last().map(|t| t.playing)
    }
}

struct Rig {
    backend: Arc<FakeBackend>,
    engine: PlaybackEngine,
    events: Arc<RecordingEvents>,
}

fn rig() -> Rig {
    let backend = FakeBackend::new();
    let engine = PlaybackEngine::with_backend(backend.clone());
    Rig {
        backend,
        engine,
        events: Arc::new(RecordingEvents::default()),
    }
}

impl Rig {
    fn events(&self) -> Arc<dyn PlaybackEvents> {
        self.events.clone()
    }

    fn play(&self, path: &Path, position_ms: u64, duration_ms: Option<u64>) -> Result<(), String> {
        self.engine
            .play(self.events(), path.to_str().unwrap(), position_ms, duration_ms)
    }

    fn seek(&self, position_ms: u64) {
        self.engine.seek(self.events(), position_ms).unwrap();
    }
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(2));
    }
    cond()
}

fn collect_until_audible_frames(backend: &FakeBackend, audible_frames: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        out.extend(backend.render_frames(128));
        let audible = out.chunks(2).filter(|f| f[0].abs() > 1e-3).count();
        if audible >= audible_frames {
            break;
        }
        thread::sleep(Duration::from_micros(300));
    }
    out
}

fn is_audible(buf: &[f32]) -> bool {
    buf.iter().any(|s| s.abs() > 1e-3)
}

#[test]
fn decodes_non_silent_stereo_at_native_rate() {
    let path = scratch("stereo_48k.wav");
    sine_wav(&path, 48_000, 2, 2, 440.0);
    let samples = decode_to_vec(&path, 0, 48_000, 2);
    assert_eq!(samples.len(), 48_000 * 2 * 2);
    assert!(peak(&samples) > 0.45, "peak {}", peak(&samples));
}

#[test]
fn resamples_and_upmixes_to_output_format() {
    let path = scratch("mono_44k.wav");
    sine_wav(&path, 44_100, 1, 2, 440.0);
    let samples = decode_to_vec(&path, 0, 48_000, 2);
    let expected = 48_000 * 2 * 2;
    assert!(samples.len().abs_diff(expected) <= 8, "len {}", samples.len());
    assert!(peak(&samples) > 0.45, "peak {}", peak(&samples));
    assert!(samples.chunks(2).all(|f| f[0] == f[1]));
}

#[test]
fn seek_offset_skips_leading_audio_and_stays_audible() {
    let path = scratch("seek_44k.wav");
    sine_wav(&path, 44_100, 2, 4, 440.0);
    let start_ms = 1_500u64;
    let start_sample = start_ms * 44_100 * 2 / 1000;
    let samples = decode_to_vec(&path, start_sample, 44_100, 2);
    let expected = (4_000 - start_ms) as usize * 44_100 * 2 / 1000;
    assert!(samples.len().abs_diff(expected) < 8_192, "len {}", samples.len());
    assert!(peak(&samples) > 0.45, "peak {}", peak(&samples));
}

#[test]
fn superseded_decode_stops_without_output() {
    let path = scratch("superseded.wav");
    sine_wav(&path, 48_000, 2, 1, 440.0);
    let mut pushed = 0usize;
    let outcome = run_decode(
        path.to_str().unwrap(),
        0,
        48_000,
        2,
        &|| false,
        &mut |samples| {
            pushed += samples.len();
            Ok(true)
        },
    )
    .unwrap();
    assert_eq!(outcome, DecodeOutcome::Superseded);
    assert_eq!(pushed, 0);
}

#[test]
fn sink_error_mid_stream_is_returned() {
    let path = scratch("sink_error.wav");
    sine_wav(&path, 48_000, 2, 2, 440.0);
    let mut calls = 0;
    let result = run_decode(
        path.to_str().unwrap(),
        0,
        48_000,
        2,
        &|| true,
        &mut |_| {
            calls += 1;
            if calls == 3 {
                Err("device gone".to_string())
            } else {
                Ok(true)
            }
        },
    );
    assert_eq!(result, Err("device gone".to_string()));
}

#[test]
fn resampler_is_packet_size_independent_and_drift_free() {
    let rate_in = 44_100u32;
    let input: Vec<f32> = (0..rate_in * 10)
        .flat_map(|n| {
            let v = f32::sin(2.0 * std::f32::consts::PI * 440.0 * n as f32 / rate_in as f32) * 0.5;
            [v, -v]
        })
        .collect();

    let mut whole = Vec::new();
    Resampler::new(rate_in, 48_000, 2).process(&input, &mut whole);

    let mut chunked = Vec::new();
    let mut rs = Resampler::new(rate_in, 48_000, 2);
    for chunk in input.chunks(1152 * 2) {
        rs.process(chunk, &mut chunked);
    }

    assert_eq!(whole.len(), chunked.len());
    assert!(whole.iter().zip(&chunked).all(|(a, b)| (a - b).abs() < 1e-6));
    let frames = whole.len() / 2;
    assert!(frames.abs_diff(480_000) <= 2, "frames {frames}");
    let max_step = whole
        .chunks(2)
        .zip(whole.chunks(2).skip(1))
        .map(|(a, b)| (a[0] - b[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(max_step < 0.08, "discontinuity {max_step}");
}

#[test]
fn engine_plays_a_file_to_the_end() {
    let rig = rig();
    let path = scratch("engine_end.wav");
    sine_wav(&path, 48_000, 2, 1, 440.0);
    rig.play(&path, 0, Some(1_000)).unwrap();

    let mut heard = 0usize;
    let mut buffers = 0;
    while buffers < 2_000 && rig.events.last_playing() != Some(false) {
        let buf = rig.backend.pump(1, Duration::from_micros(300));
        heard += buf.chunks(2).filter(|f| f[0].abs() > 1e-3).count();
        buffers += 1;
    }
    assert_eq!(rig.events.last_playing(), Some(false));
    assert!(heard > 45_000 && heard < 49_500, "heard {heard} frames");
    assert!(rig.events.errors.lock().unwrap().is_empty());
}

fn write_ramp(path: &Path, ascending: bool) {
    write_wav(path, 8_000, 2, 160_000, |n, _| {
        let x = n as f32 / 160_000.0;
        0.9 * if ascending { x } else { 1.0 - x }
    });
}

fn ramp_value(position_ms: u64, ascending: bool) -> f32 {
    let x = position_ms as f32 / 20_000.0;
    0.9 * if ascending { x } else { 1.0 - x }
}

/// Stale audio is always a much higher level than the new position (ascending ramp: seeking
/// backwards, descending ramp: seeking forwards). Fade-ins after an underrun only scale new audio
/// down, so a sample above the target level can only come from an old stream.
fn assert_no_stale(out: &[f32], wanted: f32, what: &str) {
    let settled = &out[300 * 2..];
    assert!(is_audible(settled), "{what}: no audio after seek");
    let stale: Vec<(usize, f32)> = settled
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, s)| *s > wanted + 0.08)
        .take(5)
        .collect();
    assert!(stale.is_empty(), "{what}: wanted {wanted}, stale (index, value): {stale:?}");
}

#[test]
fn seek_never_outputs_audio_from_before_the_seek() {
    let rig = rig();
    rig.engine.configure(8_000, 128).unwrap();
    let up = scratch("ramp_up_8k.wav");
    let down = scratch("ramp_down_8k.wav");
    write_ramp(&up, true);
    write_ramp(&down, false);

    for target_ms in [3_000u64, 8_000, 1_000, 12_000] {
        rig.play(&up, 19_000, Some(20_000)).unwrap();
        let before = rig.backend.pump(300, Duration::from_micros(300));
        assert!(peak(&before) > 0.8, "pre-seek peak {}", peak(&before));
        rig.seek(target_ms);
        let out = collect_until_audible_frames(&rig.backend, 6_000);
        assert_no_stale(&out, ramp_value(target_ms, true), &format!("up -> {target_ms}"));
    }

    for target_ms in [15_000u64, 9_000, 19_000] {
        rig.play(&down, 1_000, Some(20_000)).unwrap();
        let before = rig.backend.pump(300, Duration::from_micros(300));
        assert!(peak(&before) > 0.8, "pre-seek peak {}", peak(&before));
        rig.seek(target_ms);
        let out = collect_until_audible_frames(&rig.backend, 6_000);
        assert_no_stale(&out, ramp_value(target_ms, false), &format!("down -> {target_ms}"));
    }
}

#[test]
fn seek_while_tempo_and_key_are_changed_never_outputs_pre_seek_audio() {
    let carrier = scratch("carrier_ramp_8k.wav");
    write_wav(&carrier, 8_000, 2, 160_000, |n, _| {
        let amplitude = 0.9 * n as f32 / 160_000.0;
        amplitude * f32::sin(2.0 * std::f32::consts::PI * 1_000.0 * n as f32 / 8_000.0)
    });
    for (ratio, shift, lock) in [(1.16, 0.0, true), (0.9, 0.0, true), (1.08, 0.0, false), (1.0, 2.0, true)] {
        let rig = rig();
        rig.engine.configure(8_000, 128).unwrap();
        rig.engine.set_tempo(ratio);
        rig.engine.set_key_shift(shift);
        rig.engine.set_key_lock(lock);
        for target_ms in [3_000u64, 8_000, 1_000] {
            rig.play(&carrier, 19_000, Some(20_000)).unwrap();
            let before = rig.backend.pump(300, Duration::from_micros(300));
            assert!(peak(&before) > 0.5, "pre-seek peak {}", peak(&before));
            rig.engine.set_tempo(ratio * 1.01);
            rig.seek(target_ms);
            let out = collect_until_audible_frames(&rig.backend, 6_000);
            let wanted = ramp_value(target_ms, true);
            let settled = &out[300 * 2..];
            assert!(is_audible(settled), "no audio after seek");
            for window in settled.chunks(160) {
                assert!(
                    peak(window) < wanted + 0.25,
                    "tempo {ratio} st {shift} lock {lock} -> {target_ms}: wanted {wanted}, saw {}",
                    peak(window)
                );
            }
            rig.engine.set_tempo(ratio);
        }
    }
}

#[test]
fn stop_drops_tempo_and_key_shift_but_keeps_key_lock() {
    let rig = rig();
    rig.engine.set_tempo(1.12);
    rig.engine.set_key_shift(3.0);
    rig.engine.set_key_lock(false);
    rig.engine.stop().unwrap();
    let target = rig.engine.lock().tempo.target();
    assert_eq!((target.ratio, target.semitones, target.key_lock), (1.0, 0.0, false));
}

#[test]
fn tempo_commands_clamp_to_the_supported_range() {
    let rig = rig();
    rig.engine.set_tempo(9.0);
    rig.engine.set_key_shift(-99.0);
    let target = rig.engine.lock().tempo.target();
    assert_eq!((target.ratio, target.semitones), (crate::tempo::MAX_RATIO, -crate::tempo::MAX_SEMITONES));
    rig.engine.set_tempo(0.0);
    assert_eq!(rig.engine.lock().tempo.target().ratio, crate::tempo::MIN_RATIO);
}

#[test]
fn tempo_survives_a_device_reopen_and_ticks_report_it() {
    let rig = rig();
    let path = scratch("tempo_tick.wav");
    sine_wav(&path, 48_000, 2, 10, 220.0);
    rig.engine.set_tempo(1.16);
    rig.play(&path, 0, Some(10_000)).unwrap();
    rig.backend.pump(1_500, Duration::from_micros(300));
    assert!(wait_until(Duration::from_secs(1), || rig.events.tick_count() > 0));
    let ratio = rig.events.ticks.lock().unwrap().last().unwrap().tempo_ratio;
    assert!((ratio - 1.16).abs() < 0.01, "tick ratio {ratio}");

    rig.engine.configure(44_100, 256).unwrap();
    let target = rig.engine.lock().tempo.target();
    assert!((target.ratio - 1.16).abs() < 1e-6);
}

#[test]
fn key_shift_is_audible_within_a_tenth_of_a_second() {
    let rig = rig();
    let path = scratch("shift_audible.wav");
    sine_wav(&path, 48_000, 2, 10, 440.0);
    rig.play(&path, 0, Some(10_000)).unwrap();
    rig.backend.pump(400, Duration::from_micros(300));
    rig.engine.set_key_shift(12.0);
    let mut out = Vec::new();
    for _ in 0..(4_800 / 128 + 1) {
        out.extend(rig.backend.render_frames(128));
        thread::sleep(Duration::from_micros(300));
    }
    let tail: Vec<f32> = out.chunks(2).map(|f| f[0]).skip(2_600).collect();
    let crossings = tail.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    let hz = crossings as f64 * 48_000.0 / tail.len() as f64;
    assert!(hz > 440.0 * 2f64.powf(6.0 / 12.0), "pitch after 100 ms: {hz:.0} Hz");
}

#[test]
fn audio_decoded_before_the_callback_runs_is_not_flushed_after_a_seek() {
    let rig = rig();
    rig.engine.configure(8_000, 128).unwrap();
    let path = scratch("ramp_up_8k.wav");
    write_ramp(&path, true);
    rig.play(&path, 19_000, Some(20_000)).unwrap();
    rig.backend.pump(100, Duration::from_micros(300));

    rig.seek(2_000);
    thread::sleep(Duration::from_millis(200));
    let out = collect_until_audible_frames(&rig.backend, 6_000);
    let wanted = ramp_value(2_000, true);
    assert_no_stale(&out, wanted, "decode ran ahead of the callback");
    let first_full = out
        .iter()
        .copied()
        .find(|s| *s > wanted * 0.9)
        .expect("no audio at the seek position");
    assert!(first_full < wanted + 0.02, "first audio skipped ahead: {first_full} vs {wanted}");
}

#[test]
fn ticks_do_not_report_the_previous_streams_position_after_a_seek() {
    let rig = rig();
    let path = scratch("tick_seek.wav");
    sine_wav(&path, 48_000, 2, 20, 220.0);
    rig.play(&path, 10_000, Some(20_000)).unwrap();
    rig.backend.pump(400, Duration::from_micros(500));
    assert!(wait_until(Duration::from_secs(1), || rig.events.tick_count() > 0));

    let seen_before = rig.events.tick_count();
    rig.seek(0);
    rig.backend.pump(600, Duration::from_micros(500));
    let ticks = rig.events.ticks.lock().unwrap();
    let after: Vec<u64> = ticks[seen_before..].iter().map(|t| t.position_ms).collect();
    assert!(!after.is_empty());
    assert!(after.iter().all(|p| *p < 2_000), "positions after seek {after:?}");
}

#[test]
fn running_dry_fades_instead_of_cutting_off() {
    let rig = rig();
    let path = scratch("underrun_fade.wav");
    sine_wav(&path, 48_000, 2, 3, 200.0);
    rig.play(&path, 0, Some(3_000)).unwrap();
    let out = rig.backend.pump(3_000, Duration::ZERO);
    let max_step = out
        .chunks(2)
        .zip(out.chunks(2).skip(1))
        .map(|(a, b)| (a[0] - b[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(is_audible(&out));
    assert!(max_step < 0.1, "hard discontinuity {max_step}");
}

#[test]
fn channels_stay_paired_through_underruns() {
    let rig = rig();
    let path = scratch("distinct_channels.wav");
    write_wav(&path, 48_000, 2, 48_000 * 2, |n, c| {
        let base = ((n % 500) as f32 / 500.0) * 0.4 + 0.1;
        if c == 0 { base } else { base + 0.3 }
    });
    rig.play(&path, 0, Some(2_000)).unwrap();
    let mut bad = 0usize;
    let mut checked = 0usize;
    for _ in 0..3_000 {
        let buf = rig.backend.render_frames(32);
        for f in buf.chunks(2) {
            if f[0] > 0.05 && f[1] > 0.05 {
                if f[1] > f[0] {
                    checked += 1;
                } else {
                    bad += 1;
                }
            }
        }
    }
    assert!(checked > 10_000, "checked {checked}");
    assert!(rig.engine.live_thread_count() <= 2);
    assert_eq!(bad, 0, "left/right got swapped or shifted");
}

#[test]
fn truncated_file_ends_playback_instead_of_hanging() {
    let rig = rig();
    let full = scratch("trunc_full.wav");
    sine_wav(&full, 48_000, 2, 4, 440.0);
    let bytes = std::fs::read(&full).unwrap();
    let truncated = scratch("trunc_half.wav");
    std::fs::write(&truncated, &bytes[..bytes.len() / 4]).unwrap();

    rig.play(&truncated, 0, Some(4_000)).unwrap();
    let ended = wait_until(Duration::from_secs(5), || {
        rig.backend.pump(4, Duration::from_micros(300));
        rig.events.last_playing() == Some(false)
    });
    assert!(ended, "playback stayed 'playing' on a truncated file");
}

#[test]
fn corrupt_files_report_errors_without_panicking() {
    let rig = rig();
    let garbage = scratch("garbage.wav");
    std::fs::write(&garbage, (0..4096u32).map(|i| (i * 31 % 251) as u8).collect::<Vec<_>>()).unwrap();
    let empty = scratch("empty.wav");
    std::fs::write(&empty, b"").unwrap();
    let header_only = scratch("header_only.wav");
    let valid = scratch("corrupt_valid.wav");
    sine_wav(&valid, 48_000, 2, 1, 440.0);
    let bytes = std::fs::read(&valid).unwrap();
    std::fs::write(&header_only, &bytes[..44]).unwrap();
    let zero_channels = scratch("zero_channels.wav");
    let mut b = bytes.clone();
    b[22] = 0;
    b[23] = 0;
    std::fs::write(&zero_channels, &b).unwrap();
    let zero_rate = scratch("zero_rate.wav");
    let mut b = bytes.clone();
    b[24..28].fill(0);
    std::fs::write(&zero_rate, &b).unwrap();
    let huge_data = scratch("huge_data.wav");
    let mut b = bytes.clone();
    b[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    std::fs::write(&huge_data, &b).unwrap();

    for path in [&garbage, &empty, &header_only, &zero_channels, &zero_rate, &huge_data] {
        let _ = rig.play(path, 0, None);
        let before = rig.events.errors.lock().unwrap().len();
        let result = rig.play(path, 0, Some(1_000));
        rig.backend.pump(200, Duration::from_micros(200));
        let reported = result.is_err() || {
            wait_until(Duration::from_secs(2), || {
                rig.backend.pump(4, Duration::from_micros(200));
                rig.events.errors.lock().unwrap().len() > before
            }) || rig.events.last_playing() == Some(false)
        };
        assert!(reported, "{path:?} neither failed nor reported an error");
    }

    rig.play(&valid, 0, Some(1_000)).unwrap();
    let out = rig.backend.pump(500, Duration::from_micros(300));
    assert!(is_audible(&out), "engine unusable after corrupt input");
}

#[test]
fn mutated_headers_never_panic_the_decoder() {
    let valid = scratch("fuzz_valid.wav");
    sine_wav(&valid, 48_000, 2, 1, 440.0);
    let original = std::fs::read(&valid).unwrap();
    let target = scratch("fuzz_case.wav");
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut panics = 0;
    for _ in 0..400 {
        let mut bytes = original.clone();
        for _ in 0..(1 + next() % 4) {
            let at = (next() % 64) as usize;
            bytes[at] = next() as u8;
        }
        if next() % 3 == 0 {
            let cut = (next() % bytes.len() as u64) as usize;
            bytes.truncate(cut);
        }
        std::fs::write(&target, &bytes).unwrap();
        let started = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_decode(target.to_str().unwrap(), 0, 48_000, 2, &|| true, &mut |_| Ok(true))
        }));
        if result.is_err() {
            panics += 1;
        }
        assert!(started.elapsed() < Duration::from_secs(5), "decode of mutated file hung");
    }
    assert_eq!(panics, 0, "symphonia panicked on mutated input");
}

#[test]
fn device_error_is_reported_and_next_play_reopens_the_output() {
    let rig = rig();
    let path = scratch("device_error.wav");
    sine_wav(&path, 48_000, 2, 5, 330.0);
    rig.play(&path, 0, Some(5_000)).unwrap();
    rig.backend.pump(200, Duration::from_micros(300));
    assert_eq!(rig.backend.opens.load(Ordering::SeqCst), 1);

    rig.backend.inject_error("device unplugged");
    let errors = rig.events.errors.lock().unwrap().clone();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("device unplugged"));
    assert!(wait_until(Duration::from_secs(1), || rig.events.last_playing() == Some(false)));

    rig.play(&path, 0, Some(5_000)).unwrap();
    assert_eq!(rig.backend.opens.load(Ordering::SeqCst), 2);
    let out = rig.backend.pump(400, Duration::from_micros(300));
    assert!(is_audible(&out));
}

#[test]
fn failed_output_open_returns_an_error_and_recovers() {
    let backend = FakeBackend::new();
    backend.fail_open.store(true, Ordering::SeqCst);
    let engine = PlaybackEngine::with_backend(backend.clone());
    let events = Arc::new(RecordingEvents::default());
    let path = scratch("open_fail.wav");
    sine_wav(&path, 48_000, 2, 1, 330.0);

    let err = engine
        .play(events.clone(), path.to_str().unwrap(), 0, Some(1_000))
        .unwrap_err();
    assert!(err.contains("fake device unavailable"), "{err}");
    assert!(engine.configure(44_100, 128).is_err());

    backend.fail_open.store(false, Ordering::SeqCst);
    engine
        .play(events, path.to_str().unwrap(), 0, Some(1_000))
        .unwrap();
    assert!(is_audible(&backend.pump(300, Duration::from_micros(300))));
}

#[test]
fn missing_file_is_a_command_error() {
    let rig = rig();
    let err = rig
        .play(Path::new("/nonexistent/nope.mp3"), 0, Some(1_000))
        .unwrap_err();
    assert!(err.contains("nope.mp3"), "{err}");
}

#[test]
fn sample_rate_change_produces_audio_at_the_new_rate() {
    let rig = rig();
    let path = scratch("rate_change_44k.wav");
    sine_wav(&path, 44_100, 2, 1, 440.0);

    for (rate, expected) in [(44_100u32, 44_100usize), (96_000, 96_000), (48_000, 48_000)] {
        rig.engine.configure(rate, 128).unwrap();
        assert_eq!(rig.engine.audio_config().unwrap().sample_rate, rate);
        rig.play(&path, 0, Some(1_000)).unwrap();
        let mut heard = 0usize;
        let mut buffers = 0;
        let start_ticks = rig.events.tick_count();
        while buffers < 4_000 {
            let buf = rig.backend.pump(1, Duration::from_micros(100));
            heard += buf.chunks(2).filter(|f| f[0].abs() > 1e-3).count();
            buffers += 1;
            if rig.events.tick_count() > start_ticks && rig.events.last_playing() == Some(false) {
                break;
            }
        }
        assert!(
            heard.abs_diff(expected) < expected / 20,
            "rate {rate}: heard {heard} frames, expected about {expected}"
        );
    }
}

#[test]
fn device_that_ignores_the_requested_rate_is_resampled_to_its_actual_rate() {
    let rig = rig();
    *rig.backend.forced_rate.lock().unwrap() = Some(96_000);
    rig.engine.configure(48_000, 128).unwrap();
    let path = scratch("forced_rate.wav");
    sine_wav(&path, 48_000, 2, 1, 440.0);
    rig.play(&path, 0, Some(1_000)).unwrap();
    let mut heard = 0usize;
    for _ in 0..3_000 {
        let buf = rig.backend.pump(1, Duration::from_micros(100));
        heard += buf.chunks(2).filter(|f| f[0].abs() > 1e-3).count();
    }
    assert!(heard.abs_diff(96_000) < 5_000, "heard {heard}");
}

#[test]
fn stop_and_drop_join_all_threads() {
    let rig = rig();
    let path = scratch("threads.wav");
    sine_wav(&path, 48_000, 2, 10, 440.0);
    for i in 0..20 {
        rig.play(&path, i * 100, Some(10_000)).unwrap();
        rig.seek(i * 200);
    }
    rig.engine.stop().unwrap();
    assert!(wait_until(Duration::from_secs(2), || rig.engine.live_thread_count() == 0));
    assert!(rig.engine.shutdown(Duration::from_secs(1)));
}

#[test]
fn pause_keeps_position_and_resume_continues_from_it() {
    let rig = rig();
    let path = scratch("pause_resume.wav");
    sine_wav(&path, 48_000, 2, 10, 440.0);
    rig.play(&path, 2_000, Some(10_000)).unwrap();
    rig.backend.pump(750, Duration::from_micros(300));
    rig.engine.pause().unwrap();
    let paused_at = rig.engine.lock().position_ms.load(Ordering::SeqCst);
    assert!((2_000..4_000).contains(&paused_at), "paused at {paused_at}");
    let silence = rig.backend.pump(200, Duration::from_micros(200));
    assert!(silence[silence.len() / 2..].iter().all(|s| s.abs() < 1e-3));

    rig.engine.resume(rig.events()).unwrap();
    assert!(is_audible(&rig.backend.pump(300, Duration::from_micros(300))));
    assert!(wait_until(Duration::from_secs(1), || {
        rig.events.ticks.lock().unwrap().iter().any(|t| !t.playing)
    }));
}

#[test]
fn load_seek_stop_spam_from_many_threads_finishes_without_panics() {
    let rig = rig();
    let backend = rig.backend.clone();
    let engine = Arc::new(rig.engine);
    let events = rig.events.clone();

    let files: Vec<PathBuf> = {
        let a = scratch("spam_a.wav");
        sine_wav(&a, 48_000, 2, 6, 440.0);
        let b = scratch("spam_b.wav");
        sine_wav(&b, 44_100, 1, 4, 220.0);
        let c = scratch("spam_garbage.wav");
        std::fs::write(&c, vec![7u8; 500]).unwrap();
        let d = scratch("spam_trunc.wav");
        let bytes = std::fs::read(&a).unwrap();
        std::fs::write(&d, &bytes[..bytes.len() / 3]).unwrap();
        vec![a, b, c, d]
    };

    let stop = Arc::new(AtomicBool::new(false));
    let (done_tx, done_rx) = mpsc::channel();
    let mut handles = Vec::new();

    for worker in 0..6u64 {
        let engine = engine.clone();
        let events = events.clone();
        let files = files.clone();
        let stop = stop.clone();
        let done_tx = done_tx.clone();
        handles.push(thread::spawn(move || {
            let mut state = 0x9e37_79b9_7f4a_7c15u64 ^ (worker + 1).wrapping_mul(0x1234_5678_9abc);
            let mut next = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let mut ops = 0u64;
            while !stop.load(Ordering::SeqCst) {
                let file = &files[(next() % files.len() as u64) as usize];
                let pos = next() % 5_000;
                let events: Arc<dyn PlaybackEvents> = events.clone();
                match next() % 8 {
                    0 | 1 => {
                        let hint = if next() % 2 == 0 { Some(6_000) } else { None };
                        let _ = engine.play(events, file.to_str().unwrap(), pos, hint);
                    }
                    2 | 3 => {
                        let _ = engine.seek(events, pos);
                    }
                    4 => {
                        let _ = engine.stop();
                    }
                    5 => {
                        let _ = engine.pause();
                    }
                    6 => {
                        let _ = engine.resume(events);
                    }
                    _ => {
                        let _ = engine.set_position(pos);
                    }
                }
                ops += 1;
            }
            done_tx.send(ops).unwrap();
        }));
    }

    let configurer = {
        let engine = engine.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            let mut flip = false;
            while !stop.load(Ordering::SeqCst) {
                flip = !flip;
                let _ = engine.configure(if flip { 44_100 } else { 48_000 }, 128);
                thread::sleep(Duration::from_millis(80));
            }
        })
    };
    let device = {
        let backend = backend.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                backend.pump(1, Duration::from_micros(500));
            }
        })
    };

    thread::sleep(Duration::from_millis(2_500));
    stop.store(true, Ordering::SeqCst);

    let mut total_ops = 0;
    for _ in 0..6 {
        total_ops += done_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("worker deadlocked or panicked");
    }
    for h in handles {
        h.join().expect("worker panicked");
    }
    configurer.join().expect("configurer panicked");
    device.join().expect("device thread panicked");
    assert!(total_ops > 100, "only {total_ops} operations ran");

    engine.stop().unwrap();
    assert!(wait_until(Duration::from_secs(5), || engine.live_thread_count() == 0));

    engine.configure(48_000, 128).unwrap();
    engine
        .play(events.clone(), files[0].to_str().unwrap(), 0, Some(6_000))
        .unwrap();
    assert!(is_audible(&backend.pump(500, Duration::from_micros(300))));
}

#[test]
fn rapid_seek_spam_lands_on_the_last_target() {
    let rig = rig();
    rig.engine.configure(8_000, 128).unwrap();
    let path = scratch("ramp_up_8k.wav");
    write_ramp(&path, true);
    rig.play(&path, 19_000, Some(20_000)).unwrap();
    rig.backend.pump(50, Duration::from_micros(300));
    let started = Instant::now();
    for i in 0..300u64 {
        rig.seek(10_000 + (i * 37) % 9_000);
    }
    rig.seek(2_000);
    assert!(started.elapsed() < Duration::from_secs(5), "seek spam too slow");
    let out = collect_until_audible_frames(&rig.backend, 6_000);
    assert_no_stale(&out, ramp_value(2_000, true), "after seek spam");
    assert!(wait_until(Duration::from_secs(2), || rig.engine.live_thread_count() <= 2));
}

#[test]
fn render_callback_never_allocates() {
    let rig = rig();
    let path = scratch("alloc_free.wav");
    sine_wav(&path, 48_000, 2, 12, 440.0);
    rig.play(&path, 0, Some(12_000)).unwrap();
    rig.backend.pump(100, Duration::from_micros(300));
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0.0f32; 128 * 2];
    let mut audible_buffers = 0;
    let mut cpp_events = (0usize, 0usize);
    let mut run_armed = |rig: &Rig, buffers: usize| {
        COUNTING.with(|c| c.set(true));
        fourfour_stretch::alloc_count::arm();
        for _ in 0..buffers {
            rig.backend.render_into(&mut buf);
            if buf.iter().any(|s| s.abs() > 1e-3) {
                audible_buffers += 1;
            }
        }
        let (allocs, frees) = fourfour_stretch::alloc_count::disarm();
        cpp_events.0 += allocs;
        cpp_events.1 += frees;
        COUNTING.with(|c| c.set(false));
    };

    let before = ALLOCATIONS.load(Ordering::SeqCst);
    run_armed(&rig, 500);
    rig.seek(2_000);
    run_armed(&rig, 300);

    rig.engine.set_tempo(1.16);
    run_armed(&rig, 600);
    rig.engine.set_key_shift(5.0);
    run_armed(&rig, 400);
    rig.engine.set_key_lock(false);
    run_armed(&rig, 400);
    rig.engine.set_key_shift(0.0);
    rig.engine.set_tempo(0.84);
    run_armed(&rig, 400);
    rig.engine.set_key_lock(true);
    run_armed(&rig, 400);
    rig.engine.set_tempo(1.5);
    rig.engine.set_key_shift(-12.0);
    run_armed(&rig, 400);
    rig.seek(4_000);
    run_armed(&rig, 600);
    rig.engine.set_tempo(1.0);
    rig.engine.set_key_shift(0.0);
    run_armed(&rig, 800);
    rig.seek(1_000);
    rig.engine.set_tempo(1.08);
    run_armed(&rig, 600);

    rig.engine.pause().unwrap();
    run_armed(&rig, 100);
    rig.engine.stop().unwrap();
    run_armed(&rig, 100);
    let allocations = ALLOCATIONS.load(Ordering::SeqCst) - before;

    assert!(audible_buffers > 1_000, "callback never produced audio");
    assert_eq!(allocations, 0, "render callback allocated or freed memory");
    assert_eq!(cpp_events, (0, 0), "stretcher allocated or freed memory in the callback");
}

#[test]
fn latency_load_and_seek_to_first_audio() {
    let path = scratch("latency_4min_44k.wav");
    write_wav(&path, 44_100, 2, 44_100 * 240, |n, _| {
        let block = (n / (44_100 * 10)) as f32;
        f32::sin(n as f32 * 0.05) * (0.1 + 0.7 * block / 24.0)
    });
    let rig = rig();

    let mut loads = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        rig.play(&path, 0, Some(240_000)).unwrap();
        let heard = rig
            .backend
            .pump_until(Duration::from_secs(5), is_audible)
            .expect("no audio after load");
        loads.push(heard - started);
        rig.engine.stop().unwrap();
        rig.backend.pump(4, Duration::ZERO);
    }

    let mut loads_no_hint = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        rig.play(&path, 0, None).unwrap();
        let heard = rig
            .backend
            .pump_until(Duration::from_secs(5), is_audible)
            .expect("no audio after load without duration hint");
        loads_no_hint.push(heard - started);
        rig.engine.stop().unwrap();
        rig.backend.pump(4, Duration::ZERO);
    }

    rig.play(&path, 0, Some(240_000)).unwrap();
    rig.backend.pump_until(Duration::from_secs(5), is_audible).unwrap();
    let mut seeks = Vec::new();
    for target in [60_000u64, 200_000, 5_000, 120_000, 238_000, 90_000, 30_000, 180_000] {
        rig.backend.pump(4, Duration::ZERO);
        let expected = 0.1 + 0.7 * (target / 10_000) as f32 / 24.0;
        let started = Instant::now();
        rig.seek(target);
        let heard = rig
            .backend
            .pump_until(Duration::from_secs(5), |b| {
                (peak(b) - expected).abs() < 0.015
            })
            .expect("no audio of the new position after seek");
        seeks.push(heard - started);
    }

    let summarize = |name: &str, mut v: Vec<Duration>| {
        v.sort();
        let median = v[v.len() / 2];
        println!(
            "LATENCY {name}: min {:.1} ms, median {:.1} ms, max {:.1} ms (n={})",
            v[0].as_secs_f64() * 1e3,
            median.as_secs_f64() * 1e3,
            v[v.len() - 1].as_secs_f64() * 1e3,
            v.len()
        );
        median
    };
    let load_median = summarize("load(with duration) -> first audio", loads);
    let load_nohint_median = summarize("load(no duration hint) -> first audio", loads_no_hint);
    let seek_median = summarize("seek -> first audio", seeks);

    assert!(load_median < Duration::from_millis(500), "load {load_median:?}");
    assert!(load_nohint_median < Duration::from_millis(750), "load {load_nohint_median:?}");
    assert!(seek_median < Duration::from_millis(500), "seek {seek_median:?}");
}
