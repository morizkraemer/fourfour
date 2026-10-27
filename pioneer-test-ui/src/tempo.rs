//! Real-time tempo and key changes for the playback callback.
//!
//! The ring buffer stays in source time. Per callback chunk the engine takes `n_in` source frames
//! for `n` output frames (`n_in / n` is the tempo ratio, a fraction is carried) and renders them
//! through one of three paths:
//!
//! - `Identity`: ratio 1 and no key shift. The frames are copied untouched, so output is
//!   bit-identical to a player without this module.
//! - `Varispeed`: a windowed-sinc resampler. Speed and pitch move together (key lock off).
//! - `Stretch`: tempo and pitch are separate. Signalsmith Stretch changes only the duration (it
//!   is exact there), then the same resampler moves the pitch. Its own transposition is not used
//!   because it lands several cents off at low frequencies.
//!
//! The stretcher is fed from a lookahead (`feed_ahead` frames beyond the ring front) that tracks
//! its own latency, so what it plays at any moment is the source frame at the ring front. That
//! keeps the playhead, waveform and cue points exact at every ratio and makes switching between
//! paths a plain 1-chunk crossfade of aligned signals.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use fourfour_stretch::{Preset, Stretch};

pub const MIN_RATIO: f64 = 0.5;
pub const MAX_RATIO: f64 = 1.5;
pub const MAX_SEMITONES: f64 = 12.0;

pub const CHUNK_FRAMES: usize = 256;
pub const MAX_DEVICE_CHANNELS: usize = 64;

const PRIME_BURST_FRAMES: usize = CHUNK_FRAMES * 4;
const MAX_FEED_ADJUST: usize = CHUNK_FRAMES / 4;
const TAPS: usize = 64;
const HISTORY_FRAMES: usize = TAPS / 2;
const LOOKAHEAD_FRAMES: usize = TAPS / 2 + 1;
const PHASES: usize = 256;
const BANK_STEPS_PER_OCTAVE: f64 = 8.0;
const BANK_TABLES: usize = 14;
const MAX_PITCH_FACTOR: f64 = 3.2;
const MAX_MID_FRAMES: usize = (CHUNK_FRAMES as f64 * MAX_PITCH_FACTOR) as usize + 2 * LOOKAHEAD_FRAMES;
const MAX_PRIME_OUTPUT_FRAMES: usize = 8192;
const LONG_FADE_FRAMES: usize = 1024;
const LIMITER_LOOKAHEAD_SECONDS: f64 = 0.002;
const LIMITER_RELEASE_SECONDS: f64 = 0.04;
const LIMITER_CEILING: f32 = 1.0;
const SOFT_KNEE_START: f32 = 0.92;
const RATE_SMOOTHING_SECONDS: f64 = 0.03;
const SHIFT_SMOOTHING_SECONDS: f64 = 0.05;
const RATE_SNAP: f64 = 1e-6;
const SHIFT_SNAP: f64 = 1e-4;
const KERNEL_CUTOFF: f64 = 0.92;
const KAISER_BETA: f64 = 9.0;

pub fn clamp_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(MIN_RATIO, MAX_RATIO)
    } else {
        1.0
    }
}

pub fn clamp_semitones(semitones: f64) -> f64 {
    if semitones.is_finite() {
        semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES)
    } else {
        0.0
    }
}

pub fn semitone_factor(semitones: f64) -> f64 {
    (semitones / 12.0).exp2()
}

/// Frequency multiplier of the output: the key shift, plus the tempo change itself when key lock
/// is off (a turntable's behaviour).
pub fn pitch_factor(ratio: f64, semitones: f64, key_lock: bool) -> f64 {
    let shift = semitone_factor(semitones);
    if key_lock { shift } else { ratio * shift }
}

/// Source frames between what the stretcher was last fed and what it plays.
pub fn stretch_latency_frames(input_latency: usize, output_latency: usize, ratio: f64) -> f64 {
    input_latency as f64 + output_latency as f64 * ratio
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoTarget {
    pub ratio: f64,
    pub semitones: f64,
    pub key_lock: bool,
}

/// Shared between the commands and the audio callback. Atomics only.
pub struct TempoParams {
    ratio: AtomicU32,
    semitones: AtomicU32,
    key_lock: AtomicBool,
    live_ratio: AtomicU32,
}

impl TempoParams {
    pub fn new() -> Self {
        Self {
            ratio: AtomicU32::new(1.0f32.to_bits()),
            semitones: AtomicU32::new(0.0f32.to_bits()),
            key_lock: AtomicBool::new(true),
            live_ratio: AtomicU32::new(1.0f32.to_bits()),
        }
    }

    pub fn set_ratio(&self, ratio: f64) {
        self.ratio
            .store((clamp_ratio(ratio) as f32).to_bits(), Ordering::Relaxed);
    }

    pub fn set_semitones(&self, semitones: f64) {
        self.semitones
            .store((clamp_semitones(semitones) as f32).to_bits(), Ordering::Relaxed);
    }

    pub fn set_key_lock(&self, on: bool) {
        self.key_lock.store(on, Ordering::Relaxed);
    }

    /// Back to the track's own tempo and key. Key lock is a mode, not a value, so it stays.
    pub fn reset(&self) {
        self.set_ratio(1.0);
        self.set_semitones(0.0);
    }

    pub fn target(&self) -> TempoTarget {
        TempoTarget {
            ratio: f32::from_bits(self.ratio.load(Ordering::Relaxed)) as f64,
            semitones: f32::from_bits(self.semitones.load(Ordering::Relaxed)) as f64,
            key_lock: self.key_lock.load(Ordering::Relaxed),
        }
    }

    /// The smoothed ratio the callback is playing right now.
    pub fn live_ratio(&self) -> f64 {
        f32::from_bits(self.live_ratio.load(Ordering::Relaxed)) as f64
    }

    pub(crate) fn publish_live_ratio(&self, ratio: f64) {
        self.live_ratio
            .store((ratio as f32).to_bits(), Ordering::Relaxed);
    }
}

impl Default for TempoParams {
    fn default() -> Self {
        Self::new()
    }
}

fn kernel_bank() -> &'static [f32] {
    static BANK: OnceLock<Vec<f32>> = OnceLock::new();
    BANK.get_or_init(|| {
        let per_table = (PHASES + 1) * TAPS;
        let mut bank = vec![0.0f32; BANK_TABLES * per_table];
        let half = (TAPS / 2) as f64;
        let norm = bessel_i0(KAISER_BETA);
        for t in 0..BANK_TABLES {
            let cutoff = KERNEL_CUTOFF / (t as f64 / BANK_STEPS_PER_OCTAVE).exp2();
            for q in 0..=PHASES {
                let frac = q as f64 / PHASES as f64;
                let mut row = [0.0f64; TAPS];
                let mut sum = 0.0;
                for (j, slot) in row.iter_mut().enumerate() {
                    let d = (j as f64 - (half - 1.0)) - frac;
                    let x = std::f64::consts::PI * cutoff * d;
                    let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
                    let r = d / half;
                    let window = if r.abs() >= 1.0 {
                        0.0
                    } else {
                        bessel_i0(KAISER_BETA * (1.0 - r * r).sqrt()) / norm
                    };
                    *slot = sinc * window;
                    sum += *slot;
                }
                for (j, v) in row.iter().enumerate() {
                    bank[t * per_table + q * TAPS + j] = (*v / sum) as f32;
                }
            }
        }
        bank
    })
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..60 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Windowed-sinc resampler (64 taps, 256 phases, linear between rows). Reading faster than 1x
/// lowers the cutoff to stay below the new Nyquist, so nothing folds back.
pub struct Varispeed {
    bank: &'static [f32],
}

impl Varispeed {
    pub fn new() -> Self {
        Self { bank: kernel_bank() }
    }

    fn table_index(rate: f64) -> usize {
        if rate <= 1.0 {
            0
        } else {
            ((rate.log2() * BANK_STEPS_PER_OCTAVE).ceil() as usize).min(BANK_TABLES - 1)
        }
    }

    /// `input` is stereo with `HISTORY_FRAMES` of history in front. Output frame `k` reads the
    /// signal at `start + k * rate` input frames from the start of `input`. At rate 1 on a whole
    /// frame the samples pass through unchanged.
    pub fn render(&self, input: &[f32], start: f64, rate: f64, out: &mut [f32]) {
        let frames = out.len() / 2;
        if rate == 1.0 && start.fract() == 0.0 {
            let from = start as usize * 2;
            out.copy_from_slice(&input[from..from + frames * 2]);
            return;
        }
        let per_table = (PHASES + 1) * TAPS;
        let table = &self.bank[Self::table_index(rate) * per_table..][..per_table];
        let mut kernel = [0.0f32; TAPS];
        for k in 0..frames {
            let p = start + k as f64 * rate;
            let i = p as usize;
            let q = (p - i as f64) * PHASES as f64;
            let q0 = q as usize;
            let w = (q - q0 as f64) as f32;
            let row0 = &table[q0 * TAPS..(q0 + 1) * TAPS];
            let row1 = &table[(q0 + 1) * TAPS..(q0 + 2) * TAPS];
            for j in 0..TAPS {
                kernel[j] = row0[j] + w * (row1[j] - row0[j]);
            }
            let base = (i + 1 - TAPS / 2) * 2;
            let taps = &input[base..base + TAPS * 2];
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for j in 0..TAPS {
                l += taps[j * 2] * kernel[j];
                r += taps[j * 2 + 1] * kernel[j];
            }
            out[k * 2] = l;
            out[k * 2 + 1] = r;
        }
    }
}

impl Default for Varispeed {
    fn default() -> Self {
        Self::new()
    }
}

/// Brick-wall peak limiter with lookahead. The stretcher redistributes phases, so a master that
/// peaks at full scale comes out up to 5 dB over; without this the device clips it. It only acts
/// on those overshoots and passes everything under the ceiling unchanged.
struct Limiter {
    window: usize,
    delay: Vec<f32>,
    needed: Vec<f32>,
    floor: Vec<f32>,
    position: usize,
    average_sum: f64,
    gain: f32,
    release: f32,
    frames_seen: usize,
}

impl Limiter {
    fn new(sample_rate: f64) -> Self {
        let window = ((sample_rate * LIMITER_LOOKAHEAD_SECONDS) as usize).max(2);
        Self {
            window,
            delay: vec![0.0; window * 2],
            needed: vec![1.0; window],
            floor: vec![1.0; window],
            position: 0,
            average_sum: window as f64,
            gain: 1.0,
            release: (1.0 / (sample_rate * LIMITER_RELEASE_SECONDS)) as f32,
            frames_seen: 0,
        }
    }

    /// Frames the signal is late by.
    fn delay_frames(&self) -> usize {
        self.window - 1
    }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.needed.fill(1.0);
        self.floor.fill(1.0);
        self.position = 0;
        self.average_sum = self.window as f64;
        self.gain = 1.0;
        self.frames_seen = 0;
    }

    fn is_filled(&self) -> bool {
        self.frames_seen >= self.window
    }

    fn process(&mut self, stereo: &mut [f32]) {
        let window = self.window;
        for frame in stereo.chunks_exact_mut(2) {
            let peak = frame[0].abs().max(frame[1].abs());
            let needed = if peak > LIMITER_CEILING { LIMITER_CEILING / peak } else { 1.0 };
            let at = self.position;
            self.needed[at] = needed;
            let lowest = self.needed.iter().copied().fold(1.0f32, f32::min);
            self.average_sum += (lowest - self.floor[at]) as f64;
            self.floor[at] = lowest;
            let target = (self.average_sum / window as f64) as f32;
            let slow = self.gain + (1.0 - self.gain) * self.release;
            self.gain = target.min(slow);

            let slot = (at + 1) % window;
            let delayed = [self.delay[slot * 2], self.delay[slot * 2 + 1]];
            self.delay[at * 2] = frame[0];
            self.delay[at * 2 + 1] = frame[1];
            frame[0] = delayed[0] * self.gain;
            frame[1] = delayed[1] * self.gain;
            self.position = slot;
        }
        self.frames_seen += stereo.len() / 2;
    }
}

/// Rounds the last few percent below full scale so resampling overshoot cannot clip the device.
fn soft_knee(x: f32) -> f32 {
    let magnitude = x.abs();
    if magnitude <= SOFT_KNEE_START {
        return x;
    }
    let room = 1.0 - SOFT_KNEE_START;
    x.signum() * (SOFT_KNEE_START + room * ((magnitude - SOFT_KNEE_START) / room).tanh())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Identity,
    Varispeed,
    Stretch,
}

pub struct ChunkResult {
    pub valid_frames: usize,
    pub popped_frames: usize,
}

pub struct TempoEngine {
    stretch: Stretch,
    varispeed: Varispeed,
    sample_rate: f64,
    input_latency: usize,
    output_latency: usize,
    rate: f64,
    semitones: f64,
    carry: f64,
    armed: bool,
    primed: bool,
    feed_ahead: usize,
    last_path: Option<Path>,
    fade_from: Option<Path>,
    fade_total: usize,
    fade_done: usize,
    history: Vec<f32>,
    popped_total: u64,
    window: Vec<f32>,
    stretch_in: Vec<f32>,
    stretch_out: Vec<f32>,
    mid: Vec<f32>,
    mid_fifo: usize,
    mid_carry: f64,
    shifted: Vec<f32>,
    limiter: Limiter,
    varispeed_out: Vec<f32>,
    identity_dev: Vec<f32>,
    varispeed_dev: Vec<f32>,
    stretch_dev: Vec<f32>,
}

impl TempoEngine {
    pub fn new(sample_rate: u32, preset: Preset) -> Self {
        let stretch = Stretch::new(2, sample_rate as f32, preset);
        let input_latency = stretch.input_latency();
        let output_latency = stretch.output_latency();
        let dev = CHUNK_FRAMES * MAX_DEVICE_CHANNELS;
        let feed_max = CHUNK_FRAMES * 2 + PRIME_BURST_FRAMES;
        Self {
            stretch,
            varispeed: Varispeed::new(),
            sample_rate: sample_rate.max(1) as f64,
            input_latency,
            output_latency,
            rate: 1.0,
            semitones: 0.0,
            carry: 0.0,
            armed: false,
            primed: false,
            feed_ahead: 0,
            last_path: None,
            fade_from: None,
            fade_total: 0,
            fade_done: 0,
            history: vec![0.0; HISTORY_FRAMES * 2],
            popped_total: 0,
            window: vec![0.0; (feed_max + HISTORY_FRAMES + LOOKAHEAD_FRAMES) * 2],
            stretch_in: vec![0.0; feed_max * 2],
            stretch_out: vec![0.0; MAX_PRIME_OUTPUT_FRAMES * 2],
            mid: vec![0.0; (HISTORY_FRAMES + MAX_MID_FRAMES + LOOKAHEAD_FRAMES) * 2],
            mid_fifo: 0,
            mid_carry: 0.0,
            shifted: vec![0.0; CHUNK_FRAMES * 2],
            limiter: Limiter::new(sample_rate.max(1) as f64),
            varispeed_out: vec![0.0; CHUNK_FRAMES * 2],
            identity_dev: vec![0.0; dev],
            varispeed_dev: vec![0.0; dev],
            stretch_dev: vec![0.0; dev],
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    #[cfg(test)]
    pub fn last_path(&self) -> Option<Path> {
        self.last_path
    }

    /// Source frames played since the last flush, including the fractional carry.
    pub fn heard_frames(&self) -> u64 {
        self.popped_total + self.carry as u64
    }

    /// A new stream starts: forget the old one's audio. Rate and key shift keep their values.
    pub fn flush(&mut self) {
        self.disarm();
        self.carry = 0.0;
        self.last_path = None;
        self.fade_from = None;
        self.history.fill(0.0);
        self.popped_total = 0;
    }

    fn advance_parameters(&mut self, target: TempoTarget, frames: usize) {
        let seconds = frames as f64 / self.sample_rate;
        let ratio = clamp_ratio(target.ratio);
        let shift = clamp_semitones(target.semitones);
        self.rate += (ratio - self.rate) * (1.0 - (-seconds / RATE_SMOOTHING_SECONDS).exp());
        if (self.rate - ratio).abs() < RATE_SNAP {
            self.rate = ratio;
        }
        self.semitones +=
            (shift - self.semitones) * (1.0 - (-seconds / SHIFT_SMOOTHING_SECONDS).exp());
        if (self.semitones - shift).abs() < SHIFT_SNAP {
            self.semitones = shift;
        }
    }

    fn disarm(&mut self) {
        if self.armed || self.primed || self.feed_ahead > 0 {
            self.stretch.reset();
        }
        self.armed = false;
        self.primed = false;
        self.feed_ahead = 0;
        self.mid.fill(0.0);
        self.mid_fifo = 0;
        self.mid_carry = 0.0;
        self.limiter.reset();
    }

    /// `ring` is the readable region of the sample ring as two slices, interleaved with `ch`
    /// channels. `out` holds one chunk (at most `CHUNK_FRAMES` frames) in the same layout. The
    /// caller pops `popped_frames` from the ring afterwards.
    pub fn process_chunk(
        &mut self,
        ring: (&[f32], &[f32]),
        ch: usize,
        ended: bool,
        target: TempoTarget,
        out: &mut [f32],
    ) -> ChunkResult {
        let n = out.len() / ch;
        debug_assert!(n <= CHUNK_FRAMES && ch <= MAX_DEVICE_CHANNELS);
        self.advance_parameters(target, n);

        let available = (ring.0.len() + ring.1.len()) / ch;
        let identity = self.rate == 1.0 && self.semitones == 0.0;
        let stretch_wanted = !identity && (target.key_lock || self.semitones != 0.0);
        let should_arm = target.key_lock
            || self.semitones != 0.0
            || target.semitones != 0.0
            || self.last_path == Some(Path::Stretch)
            || self.fade_from == Some(Path::Stretch);

        let n_in = if identity {
            n
        } else {
            (self.carry + n as f64 * self.rate) as usize
        };
        let identity_valid = n.min(available);
        let usable = if identity {
            identity_valid > 0
        } else {
            ended || available >= n_in + LOOKAHEAD_FRAMES
        };
        if !usable {
            self.disarm();
            return ChunkResult {
                valid_frames: 0,
                popped_frames: 0,
            };
        }

        if should_arm {
            self.armed = true;
        } else {
            self.disarm();
        }
        let pitch = pitch_factor(self.rate, self.semitones, target.key_lock);
        let stretch_ran =
            self.armed && self.run_stretch(ring, ch, ended, pitch, available, n, n_in);

        let effective = if identity {
            Path::Identity
        } else if stretch_wanted && stretch_ran {
            Path::Stretch
        } else {
            Path::Varispeed
        };
        self.start_fade_if_path_changed(effective, n);
        if self.fade_from == Some(Path::Stretch) && !stretch_ran {
            self.fade_from = None;
        }
        let previous = self.fade_from;

        let valid = if identity { identity_valid } else { n };
        let popped = if identity { valid } else { n_in.min(available) };

        if effective == Path::Identity || previous == Some(Path::Identity) {
            gather_raw(ring, ch, valid, &mut self.identity_dev[..n * ch]);
        }
        let frames = n_in + LOOKAHEAD_FRAMES;
        self.window[..HISTORY_FRAMES * 2].copy_from_slice(&self.history);
        gather_stereo(
            ring,
            ch,
            0,
            frames,
            &mut self.window[HISTORY_FRAMES * 2..(HISTORY_FRAMES + frames) * 2],
        );
        if effective == Path::Varispeed || previous == Some(Path::Varispeed) {
            self.varispeed.render(
                &self.window[..(HISTORY_FRAMES + frames) * 2],
                HISTORY_FRAMES as f64 + self.carry,
                self.rate,
                &mut self.varispeed_out[..n * 2],
            );
            for sample in &mut self.varispeed_out[..n * 2] {
                *sample = soft_knee(*sample);
            }
            expand_stereo(&self.varispeed_out[..n * 2], ch, &mut self.varispeed_dev[..n * ch]);
        }

        let current = self.path_output(effective, n * ch);
        match previous {
            Some(old_path) => {
                let old = self.path_output(old_path, n * ch);
                let uncorrelated = old_path != Path::Identity && effective != Path::Identity;
                for f in 0..n {
                    let t = ((self.fade_done + f + 1) as f32 / self.fade_total as f32).min(1.0);
                    let (gain_old, gain_new) = if uncorrelated {
                        let angle = t * std::f32::consts::FRAC_PI_2;
                        (angle.cos(), angle.sin())
                    } else {
                        (1.0 - t, t)
                    };
                    for c in 0..ch {
                        out[f * ch + c] =
                            soft_knee(old[f * ch + c] * gain_old + current[f * ch + c] * gain_new);
                    }
                }
                self.fade_done += n;
                if self.fade_done >= self.fade_total {
                    self.fade_from = None;
                }
            }
            None => out[..n * ch].copy_from_slice(current),
        }

        let from = popped * 2;
        self.history
            .copy_from_slice(&self.window[from..from + HISTORY_FRAMES * 2]);
        self.carry = if identity {
            0.0
        } else {
            self.carry + n as f64 * self.rate - n_in as f64
        };
        self.popped_total += popped as u64;
        self.feed_ahead = self.feed_ahead.saturating_sub(popped);
        self.last_path = Some(effective);

        ChunkResult {
            valid_frames: valid,
            popped_frames: popped,
        }
    }

    /// Identity and resampling agree on position and phase, so they fade within the chunk. The
    /// stretcher agrees on position but not on phase, so fades to or from it are longer and
    /// equal-power. An outgoing identity path keeps playing as 1x resampling, because the ring
    /// moves at the new rate and a raw copy would jump forward at every chunk.
    fn start_fade_if_path_changed(&mut self, effective: Path, n: usize) {
        let Some(last) = self.last_path else { return };
        if last == effective || (last == Path::Varispeed && effective == Path::Identity) {
            return;
        }
        let outgoing = if last == Path::Identity { Path::Varispeed } else { last };
        if outgoing == effective {
            return;
        }
        let uncorrelated = last == Path::Stretch || effective == Path::Stretch;
        self.fade_from = Some(outgoing);
        self.fade_done = 0;
        self.fade_total = if uncorrelated { LONG_FADE_FRAMES } else { n };
    }

    fn path_output(&self, path: Path, len: usize) -> &[f32] {
        match path {
            Path::Identity => &self.identity_dev[..len],
            Path::Varispeed => &self.varispeed_dev[..len],
            Path::Stretch => &self.stretch_dev[..len],
        }
    }

    /// Stage one stretches time only, by `rate / pitch` source frames per frame it produces.
    /// Stage two (the resampler) reads its output at `pitch` frames per output frame. The
    /// stretcher output waits in a FIFO that always keeps `LOOKAHEAD_FRAMES` unread frames, so
    /// the resampler has its future taps; the lookahead is part of the feed offset.
    fn run_stretch(
        &mut self,
        ring: (&[f32], &[f32]),
        ch: usize,
        ended: bool,
        pitch: f64,
        available: usize,
        n: usize,
        n_in: usize,
    ) -> bool {
        let spacing = self.rate / pitch;
        let latency = stretch_latency_frames(self.input_latency, self.output_latency, spacing);
        let limiter_lead = self.limiter.delay_frames() as f64 * self.rate;
        let steady_ahead =
            (latency + LOOKAHEAD_FRAMES as f64 * spacing + limiter_lead).round() as usize;

        if !self.primed {
            let goal = ((latency + limiter_lead).round() as usize) + n_in;
            let room = if ended {
                usize::MAX
            } else {
                available.saturating_sub(self.feed_ahead)
            };
            let burst = PRIME_BURST_FRAMES
                .min(goal.saturating_sub(self.feed_ahead))
                .min(room);
            if burst > 0 {
                let produced = ((burst as f64 / spacing).round() as usize).clamp(1, MAX_PRIME_OUTPUT_FRAMES);
                gather_stereo(ring, ch, self.feed_ahead, burst, &mut self.stretch_in[..burst * 2]);
                self.stretch.process(
                    &mut self.stretch_in[..burst * 2],
                    &mut self.stretch_out[..produced * 2],
                );
                self.feed_ahead += burst;
            }
            if self.feed_ahead >= goal {
                let fill_in = (LOOKAHEAD_FRAMES as f64 * spacing).round() as usize;
                if ended || available >= self.feed_ahead + fill_in {
                    gather_stereo(ring, ch, self.feed_ahead, fill_in, &mut self.stretch_in[..fill_in * 2]);
                    let fifo = HISTORY_FRAMES * 2..(HISTORY_FRAMES + LOOKAHEAD_FRAMES) * 2;
                    self.stretch
                        .process(&mut self.stretch_in[..fill_in * 2], &mut self.mid[fifo]);
                    self.mid_fifo = LOOKAHEAD_FRAMES;
                    self.feed_ahead += fill_in;
                    self.primed = true;
                }
            }
            return false;
        }

        let wanted = (self.mid_carry + n as f64 * pitch) as usize;
        let produced = wanted + LOOKAHEAD_FRAMES - self.mid_fifo;
        let adjust = (steady_ahead as i64 - self.feed_ahead as i64)
            .clamp(-(MAX_FEED_ADJUST as i64), MAX_FEED_ADJUST as i64);
        let feed = (n_in as i64 + adjust).max(1) as usize;
        if !ended && available < self.feed_ahead + feed {
            self.disarm();
            return false;
        }
        gather_stereo(ring, ch, self.feed_ahead, feed, &mut self.stretch_in[..feed * 2]);
        let tail = (HISTORY_FRAMES + self.mid_fifo) * 2;
        self.stretch
            .process(&mut self.stretch_in[..feed * 2], &mut self.mid[tail..tail + produced * 2]);
        self.feed_ahead += feed;

        let readable = HISTORY_FRAMES + wanted + LOOKAHEAD_FRAMES;
        self.varispeed.render(
            &self.mid[..readable * 2],
            HISTORY_FRAMES as f64 + self.mid_carry,
            pitch,
            &mut self.shifted[..n * 2],
        );
        self.mid_carry = self.mid_carry + n as f64 * pitch - wanted as f64;
        self.mid.copy_within(
            wanted * 2..(wanted + HISTORY_FRAMES + LOOKAHEAD_FRAMES) * 2,
            0,
        );
        self.mid_fifo = LOOKAHEAD_FRAMES;
        let limiter_was_filled = self.limiter.is_filled();
        self.limiter.process(&mut self.shifted[..n * 2]);
        expand_stereo(&self.shifted[..n * 2], ch, &mut self.stretch_dev[..n * ch]);
        limiter_was_filled
    }
}

fn sample_at(ring: (&[f32], &[f32]), index: usize) -> Option<f32> {
    if index < ring.0.len() {
        Some(ring.0[index])
    } else {
        ring.1.get(index - ring.0.len()).copied()
    }
}

fn gather_stereo(ring: (&[f32], &[f32]), ch: usize, start_frame: usize, frames: usize, dst: &mut [f32]) {
    for f in 0..frames {
        let base = (start_frame + f) * ch;
        let (l, r) = match sample_at(ring, base) {
            None => (0.0, 0.0),
            Some(l) => {
                let r = if ch > 1 { sample_at(ring, base + 1).unwrap_or(0.0) } else { l };
                (l, r)
            }
        };
        dst[f * 2] = l;
        dst[f * 2 + 1] = r;
    }
}

fn gather_raw(ring: (&[f32], &[f32]), ch: usize, frames: usize, dst: &mut [f32]) {
    let samples = frames * ch;
    for (i, slot) in dst.iter_mut().enumerate() {
        *slot = if i < samples { sample_at(ring, i).unwrap_or(0.0) } else { 0.0 };
    }
}

fn expand_stereo(stereo: &[f32], ch: usize, dst: &mut [f32]) {
    let frames = stereo.len() / 2;
    for f in 0..frames {
        let frame = &mut dst[f * ch..(f + 1) * ch];
        frame.fill(0.0);
        frame[0] = stereo[f * 2];
        if ch > 1 {
            frame[1] = stereo[f * 2 + 1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_and_semitones_are_clamped() {
        assert_eq!(clamp_ratio(0.1), MIN_RATIO);
        assert_eq!(clamp_ratio(9.0), MAX_RATIO);
        assert_eq!(clamp_ratio(f64::NAN), 1.0);
        assert_eq!(clamp_ratio(1.08), 1.08);
        assert_eq!(clamp_semitones(-40.0), -MAX_SEMITONES);
        assert_eq!(clamp_semitones(13.0), MAX_SEMITONES);
        assert_eq!(clamp_semitones(f64::INFINITY), 0.0);
    }

    #[test]
    fn semitone_factor_is_equal_tempered() {
        assert_eq!(semitone_factor(0.0), 1.0);
        assert!((semitone_factor(12.0) - 2.0).abs() < 1e-12);
        assert!((semitone_factor(-12.0) - 0.5).abs() < 1e-12);
        assert!((semitone_factor(7.0) - 1.4983070768766815).abs() < 1e-12);
    }

    #[test]
    fn pitch_follows_tempo_only_without_key_lock() {
        assert_eq!(pitch_factor(1.08, 0.0, true), 1.0);
        assert_eq!(pitch_factor(1.08, 0.0, false), 1.08);
        assert!((pitch_factor(1.08, 12.0, true) - 2.0).abs() < 1e-12);
        assert!((pitch_factor(1.08, 12.0, false) - 2.16).abs() < 1e-12);
    }

    #[test]
    fn stretch_latency_grows_with_the_ratio() {
        assert_eq!(stretch_latency_frames(2880, 2880, 1.0), 5760.0);
        assert_eq!(stretch_latency_frames(2880, 2880, 1.5), 7200.0);
        assert!((stretch_latency_frames(2880, 2880, 0.5) - 4320.0).abs() < 1e-9);
    }

    #[test]
    fn params_reset_keeps_key_lock_and_clamps_inputs() {
        let params = TempoParams::new();
        params.set_key_lock(false);
        params.set_ratio(1.16);
        params.set_semitones(30.0);
        let target = params.target();
        assert!((target.ratio - 1.16).abs() < 1e-6);
        assert_eq!(target.semitones, MAX_SEMITONES);
        params.reset();
        let target = params.target();
        assert_eq!((target.ratio, target.semitones, target.key_lock), (1.0, 0.0, false));
        params.set_ratio(f64::NAN);
        assert_eq!(params.target().ratio, 1.0);
    }

    #[test]
    fn kernel_tables_sum_to_one_and_lower_the_cutoff_for_fast_reads() {
        let bank = kernel_bank();
        let per_table = (PHASES + 1) * TAPS;
        for t in 0..BANK_TABLES {
            for q in [0, 37, 128, 255] {
                let row = &bank[t * per_table + q * TAPS..][..TAPS];
                let sum: f32 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-5, "table {t} row {q}: {sum}");
            }
        }
        assert_eq!(Varispeed::table_index(0.7), 0);
        assert_eq!(Varispeed::table_index(1.0), 0);
        assert_eq!(Varispeed::table_index(2.0), 8);
        assert_eq!(Varispeed::table_index(3.2), 13);
    }
}
