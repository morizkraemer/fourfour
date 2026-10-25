// Derived from baken, crates/baken-export/src/anlz/generate/waveform.rs
// Copyright (c) 2025 Masanari Higashi, MIT License (see NOTICE at the repository root).
// Changes for fourfour: no decoder or section assembly here, tunable crossovers and
// gains in `Tuning`, serde on the output, fourfour tests.

//! Waveform data for the Pioneer ANLZ sections, computed from decoded audio.
//!
//! One pass over the audio ([`Meter`]) reduces the track to columns of 1/150 s.
//! [`analyze`] turns the columns into every waveform the USB export writes, so
//! `PWAV`, `PWV2`, `PWV3`, `PWV4`, `PWV5`, `PWV6` and `PWV7` always come from the
//! same band data. The app draws the same [`Waveforms`] values the stick gets.
//!
//! Column grid: column `i` covers samples `[round(i * rate / 150), round((i + 1) * rate / 150))`
//! of the mono mix `(L + R) / 2`. The previews (400, 100 and 1200 columns) are
//! aggregates of those columns.

use serde::{Deserialize, Serialize};

use crate::models::{ColorWaveform, ExactColumns, WaveformPreview};

pub const COLUMNS_PER_SECOND: f64 = 150.0;
pub const PWAV_COLUMNS: usize = 400;
pub const PWV2_COLUMNS: usize = 100;
pub const OVERVIEW_COLUMNS: usize = 1200;
pub const PWV7_MAX: u8 = 127;
pub const PWV6_MAX: u8 = 127;
const PWAV_FLOOR: u8 = 2;
const PWAV_SPAN: f64 = 23.0;
const PWV2_FLOOR: u8 = 1;
const PWV2_SPAN: f64 = 14.0;

/// Every waveform of one track, in the units of the ANLZ sections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waveforms {
    /// `PWV3`: 3-bit whiteness (high bits), 5-bit height per 1/150 s.
    pub pwv3: Vec<u8>,
    /// `PWV5`: `rrrgggbbbhhhhh00` per 1/150 s (3-bit low, mid, high level; 5-bit height).
    pub pwv5: Vec<u16>,
    /// `PWV7`: low, mid, high per 1/150 s, 0..=127.
    pub pwv7: Vec<[u8; 3]>,
    /// `PWVC`: low, mid, high gain.
    pub gains: [u16; 3],
    /// `PWAV`: 400 columns, 3-bit whiteness (high bits), 5-bit height.
    pub pwav: Vec<u8>,
    /// `PWV2`: 100 columns, 4-bit height.
    pub pwv2: Vec<u8>,
    /// `PWV6`: 1200 columns of low, mid, high.
    pub pwv6: Vec<[u8; 3]>,
    /// `PWV4`: 1200 columns of six bytes.
    pub pwv4: Vec<[u8; 6]>,
}

/// Band crossovers, envelope release and gain rules. [`Tuning::default`] is the
/// fit against Rekordbox documented in `docs/architecture/waveforms-3band-2026-10.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// Second-order Butterworth low-pass whose peak, relative to the column peak, is the whiteness.
    pub whiteness_lowpass_hz: f64,
    /// Low band: low-pass.
    pub low_hz: f64,
    /// Mid band: high-pass at `mid_low_hz`, then low-pass at `mid_high_hz`.
    pub mid_low_hz: f64,
    pub mid_high_hz: f64,
    /// High band: high-pass.
    pub high_hz: f64,
    /// Per-column release of the band envelopes (low, mid, high).
    pub release: [f64; 3],
    /// Gain per band: `clamp(round(target / max_envelope), floor, cap)`.
    pub gain_target: [f64; 3],
    pub gain_floor: [u16; 3],
    pub gain_cap: [u16; 3],
    /// Byte scale applied on top of the gain.
    pub band_scale: [f64; 3],
    /// Exponent of the band curve: `127 * (value / 127)^curve`, so transients stand out from the body.
    #[serde(default = "linear_curve")]
    pub curve: [f64; 3],
    /// `PWV6` is the column mean of `PWV7` times these (only when the audio's own columns are not at hand,
    /// see `overview_from_columns`).
    pub pwv6_scale: [f64; 3],
    /// `PWV6` from the audio: each of the 1200 columns is the mean of the raw band peaks over a trailing window of
    /// this many columns' width (a column is `1 / 1200` of the track), per band.
    #[serde(default = "default_overview_window")]
    pub overview_window: [f64; 3],
    /// `byte = scale * mean^gamma / p99^delta`, p99 being the 99th percentile of the band's 1200 means.
    #[serde(default = "default_overview_scale")]
    pub overview_scale: [f64; 3],
    #[serde(default = "default_overview_gamma")]
    pub overview_gamma: [f64; 3],
    #[serde(default = "default_overview_delta")]
    pub overview_delta: [f64; 3],
    /// `PWV4` bytes 3..=5 are `PWV6` times these.
    pub pwv4_scale: [f64; 3],
}

fn linear_curve() -> [f64; 3] {
    [1.0; 3]
}

fn default_overview_window() -> [f64; 3] {
    [2.2, 1.8, 1.4]
}

fn default_overview_scale() -> [f64; 3] {
    [51.7, 36.4, 87.0]
}

fn default_overview_gamma() -> [f64; 3] {
    [1.0, 0.91, 1.04]
}

fn default_overview_delta() -> [f64; 3] {
    [0.63, 0.92, 0.67]
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            whiteness_lowpass_hz: 150.0,
            low_hz: 300.0,
            mid_low_hz: 250.0,
            mid_high_hz: 1200.0,
            high_hz: 3000.0,
            release: [0.965, 0.925, 0.90],
            gain_target: [77.0, 104.0, 140.0],
            gain_floor: [80, 80, 80],
            gain_cap: [300, 300, 500],
            band_scale: [1.265, 1.17, 1.0],
            curve: [1.0, 1.0, 2.08],
            pwv6_scale: [0.56, 0.71, 1.2],
            overview_window: default_overview_window(),
            overview_scale: default_overview_scale(),
            overview_gamma: default_overview_gamma(),
            overview_delta: default_overview_delta(),
            pwv4_scale: [2.4, 1.6, 1.1],
        }
    }
}

/// Direct form I biquad.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a0: f64, a1: f64, a2: f64) -> Self {
        Biquad {
            b: [b[0] / a0, b[1] / a0, b[2] / a0],
            a: [a1 / a0, a2 / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    /// RBJ cookbook low-pass, Q = 1/sqrt(2).
    fn lowpass(fc: f64, rate: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * fc / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        Self::new(
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        )
    }

    fn highpass(fc: f64, rate: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * fc / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        Self::new(
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        )
    }

    #[inline]
    fn step(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Per-column measurements at 150 columns per second.
#[derive(Debug, Default, Clone)]
pub struct Column {
    peak: f64,
    white: f64,
    band: [f64; 3],
    sumsq: f64,
    samples: u32,
}

/// A decoded track reduced to its columns.
#[derive(Debug, Clone, Default)]
pub struct Measured {
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: u64,
    pub tuning: Tuning,
    columns: Vec<Column>,
}

impl Measured {
    pub fn duration_ms(&self) -> f64 {
        self.frames as f64 * 1000.0 / self.sample_rate.max(1) as f64
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Peak of each band's filter output per column, before any release or gain.
    pub fn band_columns(&self) -> Vec<[f64; 3]> {
        self.columns.iter().map(|c| c.band).collect()
    }
}

/// Measures interleaved audio into columns as it arrives. Only the columns are
/// held, a few MB for a whole track.
pub struct Meter {
    sample_rate: u32,
    rate: f64,
    channels: usize,
    frames: u64,
    tuning: Tuning,
    columns: Vec<Column>,
    current: Column,
    next_edge: u64,
    /// Whiteness low-pass, low, mid high-pass, mid low-pass, high.
    filters: [Biquad; 5],
}

impl Meter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self::with_tuning(sample_rate, channels, Tuning::default())
    }

    pub fn with_tuning(sample_rate: u32, channels: usize, tuning: Tuning) -> Self {
        let rate = sample_rate.max(1) as f64;
        Meter {
            sample_rate,
            rate,
            channels,
            frames: 0,
            tuning,
            columns: Vec::new(),
            current: Column::default(),
            next_edge: (rate / COLUMNS_PER_SECOND).round() as u64,
            filters: [
                Biquad::lowpass(tuning.whiteness_lowpass_hz, rate),
                Biquad::lowpass(tuning.low_hz, rate),
                Biquad::highpass(tuning.mid_low_hz, rate),
                Biquad::lowpass(tuning.mid_high_hz, rate),
                Biquad::highpass(tuning.high_hz, rate),
            ],
        }
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        let channels = self.channels.max(1);
        let [mut white, mut low, mut mid_hp, mut mid_lp, mut high] = self.filters;
        let (mut frames, mut next_edge) = (self.frames, self.next_edge);
        let mut c = std::mem::take(&mut self.current);
        for frame in interleaved.chunks_exact(channels) {
            while frames >= next_edge {
                self.columns.push(std::mem::take(&mut c));
                next_edge = ((self.columns.len() + 1) as f64 * self.rate / COLUMNS_PER_SECOND)
                    .round() as u64;
            }
            let x = frame.iter().map(|&s| s as f64).sum::<f64>() / channels as f64;
            c.peak = c.peak.max(x.abs());
            c.white = c.white.max(white.step(x).abs());
            c.band[0] = c.band[0].max(low.step(x).abs());
            c.band[1] = c.band[1].max(mid_lp.step(mid_hp.step(x)).abs());
            c.band[2] = c.band[2].max(high.step(x).abs());
            c.sumsq += x * x;
            c.samples += 1;
            frames += 1;
        }
        self.current = c;
        self.filters = [white, low, mid_hp, mid_lp, high];
        (self.frames, self.next_edge) = (frames, next_edge);
    }

    /// `ceil(frames * 150 / rate)` columns: a last column that starts at the end
    /// of the audio stays empty.
    pub fn finish(mut self) -> Measured {
        let n = ((self.frames as f64 * COLUMNS_PER_SECOND / self.rate).ceil() as usize).max(1);
        self.columns.push(self.current);
        self.columns.resize(n, Column::default());
        Measured {
            sample_rate: self.sample_rate,
            channels: self.channels,
            frames: self.frames,
            tuning: self.tuning,
            columns: self.columns,
        }
    }
}

/// `floor(31.5 * (peak / track peak)^2)`, rekordbox's height rule.
fn height(peak: f64, track_peak: f64) -> u8 {
    if track_peak <= 0.0 {
        return 0;
    }
    let q = peak / track_peak;
    ((31.5 * q * q).floor() as u8).min(31)
}

/// `7 - floor(8 * lowpassed peak / peak)`; silence counts as white.
fn whiteness(low: f64, peak: f64) -> u8 {
    if peak <= 0.0 {
        return 7;
    }
    let r = (low / peak).clamp(0.0, 1.0);
    (7 - ((8.0 * r).floor() as i32).min(7)) as u8
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Split `n` items into `parts` consecutive ranges, each at least one item long
/// (ranges overlap when `n < parts`).
fn ranges(n: usize, parts: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
    (0..parts).map(move |k| {
        let a = (k * n / parts).min(n - 1);
        let b = ((k + 1) * n / parts).clamp(a + 1, n);
        a..b
    })
}

pub fn analyze(audio: &Measured) -> Waveforms {
    let t = &audio.tuning;
    let columns = &audio.columns;
    let n = columns.len();
    if n == 0 {
        return Waveforms::default();
    }
    let track_peak = columns.iter().map(|c| c.peak).fold(0.0, f64::max);

    let heights: Vec<u8> = columns.iter().map(|c| height(c.peak, track_peak)).collect();
    let whites: Vec<u8> = columns.iter().map(|c| whiteness(c.white, c.peak)).collect();
    let pwv3: Vec<u8> = heights.iter().zip(&whites).map(|(h, w)| (w << 5) | h).collect();

    let mut env = vec![[0.0f64; 3]; n];
    for (i, c) in columns.iter().enumerate() {
        for j in 0..3 {
            let prev = if i == 0 { 0.0 } else { env[i - 1][j] * t.release[j] };
            env[i][j] = c.band[j].max(prev);
        }
    }
    let mut gains = [0u16; 3];
    for j in 0..3 {
        let max = env.iter().map(|e| e[j]).fold(0.0, f64::max);
        gains[j] = if max > 0.0 {
            ((t.gain_target[j] / max).round().min(u16::MAX as f64) as u16)
                .clamp(t.gain_floor[j], t.gain_cap[j])
        } else {
            t.gain_floor[j]
        };
    }
    let pwv7: Vec<[u8; 3]> = env
        .iter()
        .map(|e| {
            let mut out = [0u8; 3];
            for j in 0..3 {
                let linear = (t.band_scale[j] * gains[j] as f64 * e[j]).min(PWV7_MAX as f64);
                out[j] = (PWV7_MAX as f64 * (linear / PWV7_MAX as f64).powf(t.curve[j])).round() as u8;
            }
            out
        })
        .collect();

    let pwv5: Vec<u16> = columns
        .iter()
        .zip(&heights)
        .map(|(c, &h)| pwv5_entry(c.band, h))
        .collect();

    let rms_over = |r: std::ops::Range<usize>| -> f64 {
        let (sumsq, samples) = columns[r]
            .iter()
            .fold((0.0, 0u32), |(s, k), c| (s + c.sumsq, k + c.samples));
        if samples == 0 { 0.0 } else { (sumsq / samples as f64).sqrt() }
    };
    let (pwav, pwv2) = previews(n, &whites, &rms_over);

    let raw: Vec<[f64; 3]> = columns.iter().map(|c| c.band).collect();
    let pwv6 = overview_from_columns(&raw, t);
    let pwv4 = pwv4_from(&pwv6, t);

    Waveforms { pwv3, pwv5, pwv7, gains, pwav, pwv2, pwv6, pwv4 }
}

/// `PWAV` (400 columns) and `PWV2` (100 columns) from per-range loudness `rms_over`.
/// Rekordbox never writes a `PWAV` height below 2 or a `PWV2` height of 0 for a track with sound.
fn previews(
    n: usize,
    whites: &[u8],
    rms_over: &dyn Fn(std::ops::Range<usize>) -> f64,
) -> (Vec<u8>, Vec<u8>) {
    let rms400: Vec<f64> = ranges(n, PWAV_COLUMNS).map(rms_over).collect();
    let rms_max = rms400.iter().cloned().fold(0.0, f64::max);
    let whites_f: Vec<f64> = whites.iter().map(|&w| w as f64).collect();
    let white200: Vec<u8> = ranges(n, PWAV_COLUMNS / 2)
        .map(|r| mean(&whites_f[r]).round() as u8)
        .collect();
    let pwav = rms400
        .iter()
        .enumerate()
        .map(|(k, &rms)| {
            let h = if rms_max > 0.0 {
                PWAV_FLOOR + (PWAV_SPAN * rms / rms_max).round() as u8
            } else {
                0
            };
            (white200[k / 2].min(7) << 5) | h.min(31)
        })
        .collect();
    let rms100: Vec<f64> = ranges(n, PWV2_COLUMNS).map(rms_over).collect();
    let rms100_max = rms100.iter().cloned().fold(0.0, f64::max);
    let pwv2 = rms100
        .iter()
        .map(|&rms| {
            if rms100_max > 0.0 {
                PWV2_FLOOR + (PWV2_SPAN * (rms / rms100_max).sqrt()).round() as u8
            } else {
                0
            }
        })
        .collect();
    (pwav, pwv2)
}

/// Colour: each band's share of the strongest band, on a square root so faint
/// bands still tint the column. Silence is neutral (7, 7, 7), as Rekordbox writes it.
fn pwv5_entry(band: [f64; 3], height: u8) -> u16 {
    let max = band.iter().cloned().fold(0.0, f64::max);
    let level = |b: f64| -> u16 {
        if max <= 0.0 { 7 } else { (7.0 * (b / max).sqrt()).round().min(7.0) as u16 }
    };
    (level(band[0]) << 13) | (level(band[1]) << 10) | (level(band[2]) << 7) | ((height as u16) << 2)
}

/// The band value before the curve: the overview is a mean of these, not of the curved bytes.
fn undo_curve(byte: u8, curve: f64) -> f64 {
    PWV7_MAX as f64 * (byte as f64 / PWV7_MAX as f64).powf(1.0 / curve)
}

/// `PWV6` from the audio's own columns (the raw peak of each band's filter output per 1/150 s), the way Rekordbox's
/// overview relates to the audio: a trailing mean over about two columns' width per band, then a power of that
/// scaled by the band's 99th percentile, so the white spikes and the breakdowns keep their proportions.
/// Fit against 40 Rekordbox 7.2 tracks: per-band correlation with its `PWV6` 0.996 / 0.990 / 0.990 on the held-out half
/// (the mean of the `PWV7` bytes gave 0.87 / 0.75 / 0.80).
pub fn overview_from_columns(raw: &[[f64; 3]], t: &Tuning) -> Vec<[u8; 3]> {
    let n = raw.len();
    if n == 0 {
        return Vec::new();
    }
    let per = n as f64 / OVERVIEW_COLUMNS as f64;
    let mut out = vec![[0u8; 3]; OVERVIEW_COLUMNS];
    for j in 0..3 {
        let mut prefix = Vec::with_capacity(n + 1);
        prefix.push(0.0);
        for c in raw {
            prefix.push(prefix[prefix.len() - 1] + c[j]);
        }
        let means: Vec<f64> = (0..OVERVIEW_COLUMNS)
            .map(|k| {
                let hi = (k as f64 + 1.0) * per;
                let lo = hi - t.overview_window[j] * per;
                let a = (lo.floor().max(0.0) as usize).min(n - 1);
                let b = (hi.ceil().max(1.0) as usize).min(n).max(a + 1);
                (prefix[b] - prefix[a]) / (b - a) as f64
            })
            .collect();
        let mut sorted = means.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = 0.99 * (sorted.len() - 1) as f64;
        let lo = at.floor() as usize;
        let hi = (lo + 1).min(sorted.len() - 1);
        let p99 = sorted[lo] + (sorted[hi] - sorted[lo]) * (at - lo as f64);
        for (k, m) in means.iter().enumerate() {
            let v = if p99 > 0.0 && *m > 0.0 {
                t.overview_scale[j] * m.powf(t.overview_gamma[j]) / p99.powf(t.overview_delta[j])
            } else {
                0.0
            };
            out[k][j] = v.round().min(PWV6_MAX as f64) as u8;
        }
    }
    out
}

/// `PWV6` and `PWV4` from the `PWV7` bytes alone (the export path when the audio's columns are not at hand).
fn overviews(pwv7: &[[u8; 3]], t: &Tuning) -> (Vec<[u8; 3]>, Vec<[u8; 6]>) {
    let n = pwv7.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let pwv6: Vec<[u8; 3]> = ranges(n, OVERVIEW_COLUMNS)
        .map(|r| {
            let mut out = [0u8; 3];
            for j in 0..3 {
                let m = pwv7[r.clone()].iter().map(|b| undo_curve(b[j], t.curve[j])).sum::<f64>() / r.len() as f64;
                out[j] = (t.pwv6_scale[j] * m).round().min(PWV6_MAX as f64) as u8;
            }
            out
        })
        .collect();
    let pwv4 = pwv4_from(&pwv6, t);
    (pwv6, pwv4)
}

/// `PWV4` from `PWV6`.
fn pwv4_from(pwv6: &[[u8; 3]], t: &Tuning) -> Vec<[u8; 6]> {
    pwv6.iter()
        .map(|b6| {
            let mut b = [0u8; 6];
            for j in 0..3 {
                b[3 + j] = (t.pwv4_scale[j] * b6[j] as f64).round().min(127.0) as u8;
            }
            let (r, g, bl) = (b[3] as f64, b[4] as f64, b[5] as f64);
            let max = r.max(g).max(bl);
            if max > 0.0 {
                b[2] = (0.91 * (r * r + g * g + bl * bl).sqrt()).round().min(127.0) as u8;
                b[0] = (0.88 * max + 38.0).round().min(127.0) as u8;
                b[1] = (203.0 - 0.49 * b[0] as f64).round().clamp(0.0, 255.0) as u8;
            }
            b
        })
        .collect()
}

/// How the sections other than `PWV7` follow from the band bytes when only those
/// are known (the case for an `AnalysisResult`, which carries `PWV7`-style bands).
/// [`Derivation::default`] is the fit against Rekordbox documented in
/// `docs/architecture/waveforms-3band-2026-10.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Derivation {
    /// Column loudness is `(sum(weight * band^power))^(1/power)`; height is `31.5 * (loudness / track max)^2`.
    pub loudness_weight: [f64; 3],
    pub loudness_power: f64,
    /// Whiteness is `7 - floor(8 * clamp(bass_share_gain * low / (low + mid + high)))`.
    pub bass_share_gain: f64,
    /// Relative weight of each band before the `PWV5` levels are taken.
    pub level_weight: [f64; 3],
    /// `PWVC` gains written when the real ones are unknown.
    pub gains: [u16; 3],
}

impl Default for Derivation {
    fn default() -> Self {
        Derivation {
            loudness_weight: [3.254, 0.505, 0.964],
            loudness_power: 1.1009,
            bass_share_gain: 1.0997,
            level_weight: [1.69, 0.350, 2.959],
            gains: [85, 134, 135],
        }
    }
}

/// Resamples `detail` to `columns` entries (nearest earlier column), all in 64-bit math.
pub fn resample_columns(detail: &[[u8; 3]], columns: usize) -> Vec<[u8; 3]> {
    resample(detail, columns, [0; 3])
}

fn resample<T: Copy>(source: &[T], columns: usize, empty: T) -> Vec<T> {
    if source.is_empty() {
        return vec![empty; columns];
    }
    (0..columns)
        .map(|i| source[(i as u64 * source.len() as u64 / columns as u64) as usize])
        .collect()
}

impl Waveforms {
    /// Every section from `PWV7`-style band bytes (low, mid, high, 0..=127) at 150 per second.
    pub fn from_bands(pwv7: Vec<[u8; 3]>, tuning: &Tuning, derivation: &Derivation) -> Waveforms {
        let n = pwv7.len();
        if n == 0 {
            return Waveforms { gains: derivation.gains, ..Waveforms::default() };
        }
        let d = derivation;
        let loudness: Vec<f64> = pwv7
            .iter()
            .map(|b| {
                (0..3)
                    .map(|j| d.loudness_weight[j] * (b[j] as f64).powf(d.loudness_power))
                    .sum::<f64>()
                    .powf(1.0 / d.loudness_power)
            })
            .collect();
        let loudness_max = loudness.iter().cloned().fold(0.0, f64::max);
        let heights: Vec<u8> = loudness.iter().map(|&l| height(l, loudness_max)).collect();
        let whites: Vec<u8> = pwv7
            .iter()
            .map(|b| {
                let sum = b[0] as f64 + b[1] as f64 + b[2] as f64;
                if sum <= 0.0 {
                    return 7;
                }
                let share = (d.bass_share_gain * b[0] as f64 / sum).clamp(0.0, 1.0);
                (7 - ((8.0 * share).floor() as i32).min(7)) as u8
            })
            .collect();
        let pwv3: Vec<u8> = heights.iter().zip(&whites).map(|(h, w)| (w << 5) | h).collect();
        let pwv5: Vec<u16> = pwv7
            .iter()
            .zip(&heights)
            .map(|(b, &h)| {
                pwv5_entry(
                    [
                        b[0] as f64 * d.level_weight[0],
                        b[1] as f64 * d.level_weight[1],
                        b[2] as f64 * d.level_weight[2],
                    ],
                    h,
                )
            })
            .collect();

        let energy: Vec<f64> = loudness.iter().map(|l| l * l).collect();
        let (pwav, pwv2) = previews(n, &whites, &|r| mean(&energy[r]).sqrt());

        let (pwv6, pwv4) = overviews(&pwv7, tuning);
        Waveforms { pwv3, pwv5, pwv7, gains: d.gains, pwav, pwv2, pwv6, pwv4 }
    }
}

impl Waveforms {
    /// The band data an `AnalysisResult` carries: `PWV7` bytes as `detail`, `PWV6` bytes as
    /// `overview`, and the `PWV3` / `PWV5` / `PWVC` values as `exact`.
    pub fn color_waveform(&self) -> ColorWaveform {
        ColorWaveform {
            detail: self.pwv7.clone(),
            overview: self.pwv6.clone(),
            exact: Some(ExactColumns {
                pwv3: self.pwv3.clone(),
                pwv5: self.pwv5.clone(),
                gains: self.gains,
            }),
        }
    }

    /// Replaces the derived `PWV6` (and `PWV4`, which follows it) with the overview measured from the audio.
    pub fn with_overview(mut self, pwv6: &[[u8; 3]], tuning: &Tuning) -> Waveforms {
        self.pwv6 = pwv6.iter().map(|b| b.map(|v| v.min(PWV6_MAX))).collect();
        self.pwv4 = pwv4_from(&self.pwv6, tuning);
        self
    }

    /// Replaces the derived `PWV3`, `PWV5` and `PWVC` values with the measured ones.
    pub fn with_exact(mut self, exact: &ExactColumns) -> Waveforms {
        let columns = self.pwv7.len();
        self.pwv3 = resample(&exact.pwv3, columns, 0);
        self.pwv5 = resample(&exact.pwv5, columns, 0);
        self.gains = exact.gains;
        self
    }

    /// The mono `PWAV` preview an `AnalysisResult` carries.
    pub fn preview(&self) -> Option<WaveformPreview> {
        let data: [u8; PWAV_COLUMNS] = self.pwav.clone().try_into().ok()?;
        Some(WaveformPreview { data })
    }

    /// Colour sections for an analysis that has no band data: heights follow the mono
    /// `PWAV` bytes and every band is equal, so the waveform is neutral. `pwv7` and `pwv6` stay empty.
    pub fn from_mono_preview(preview: &[u8; PWAV_COLUMNS], columns: usize, tuning: &Tuning) -> Waveforms {
        let at = |i: usize| preview[(i as u64 * PWAV_COLUMNS as u64 / columns.max(1) as u64) as usize];
        let pwv3: Vec<u8> = (0..columns).map(|i| (at(i) & 0xe0) | (at(i) & 0x1f)).collect();
        let pwv5: Vec<u16> = (0..columns)
            .map(|i| (7 << 13) | (7 << 10) | (7 << 7) | (((at(i) & 0x1f) as u16) << 2))
            .collect();
        let flat: Vec<[u8; 3]> = (0..columns)
            .map(|i| {
                let level = ((at(i) & 0x1f) as u32 * PWV7_MAX as u32 / 31) as u8;
                [level; 3]
            })
            .collect();
        let (_, pwv4) = overviews(&flat, tuning);
        let heights: Vec<f64> = preview.iter().map(|b| (b & 0x1f) as f64).collect();
        let peak = heights.iter().cloned().fold(0.0, f64::max);
        let pwv2 = ranges(PWAV_COLUMNS, PWV2_COLUMNS)
            .map(|r| {
                let m = mean(&heights[r]);
                if peak > 0.0 { PWV2_FLOOR + (PWV2_SPAN * (m / peak).sqrt()).round() as u8 } else { 0 }
            })
            .collect();
        Waveforms {
            pwv3,
            pwv5,
            pwv7: Vec::new(),
            gains: Derivation::default().gains,
            pwav: preview.to_vec(),
            pwv2,
            pwv6: Vec::new(),
            pwv4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_samples(rate: u32, seconds: f64, hz: f64, amp: f64) -> Vec<f32> {
        let frames = (rate as f64 * seconds) as usize;
        (0..frames)
            .flat_map(|i| {
                let v = (amp * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()) as f32;
                [v, v]
            })
            .collect()
    }

    fn metered(rate: u32, channels: usize, chunks: &[&[f32]]) -> Measured {
        let mut m = Meter::new(rate, channels);
        for c in chunks {
            m.push(c);
        }
        m.finish()
    }

    fn tone(rate: u32, seconds: f64, hz: f64, amp: f64) -> Measured {
        metered(rate, 2, &[&tone_samples(rate, seconds, hz, amp)])
    }

    #[test]
    fn overview_keeps_spikes_and_breakdowns_of_the_audio() {
        let t = Tuning::default();
        let mut raw = vec![[0.2, 0.1, 0.02]; 12_000];
        for c in raw.iter_mut().skip(6_000).take(600) {
            *c = [0.0; 3];
        }
        for c in raw.iter_mut().skip(3_000).take(3) {
            c[2] = 0.5;
        }
        let o = overview_from_columns(&raw, &t);
        assert_eq!(o.len(), OVERVIEW_COLUMNS);
        assert!(o[300][0] > 20 && o[300][1] > 10, "body of the track: {:?}", o[300]);
        assert_eq!(o[630], [0, 0, 0], "breakdown");
        let spike = o[301][2].max(o[302][2]).max(o[300][2]);
        assert!(spike as f64 > 2.0 * o[200][2] as f64, "a hat spike stands out of the body: {spike} against {}", o[200][2]);
        assert!(o.iter().all(|b| b.iter().all(|&v| v <= PWV6_MAX)));
    }

    #[test]
    fn overview_of_nothing_is_empty_and_of_silence_is_silent() {
        let t = Tuning::default();
        assert!(overview_from_columns(&[], &t).is_empty());
        assert!(overview_from_columns(&vec![[0.0; 3]; 500], &t).iter().all(|b| *b == [0, 0, 0]));
    }

    #[test]
    fn column_count_and_lengths() {
        let w = analyze(&tone(44100, 2.0, 440.0, 0.5));
        assert_eq!((w.pwv3.len(), w.pwv5.len(), w.pwv7.len()), (300, 300, 300));
        assert_eq!(
            (w.pwav.len(), w.pwv2.len(), w.pwv6.len(), w.pwv4.len()),
            (400, 100, 1200, 1200)
        );
    }

    #[test]
    fn steady_tone_is_full_height_and_bass_is_not_white() {
        let w = analyze(&tone(48000, 1.0, 1000.0, 0.8));
        assert!(w.pwv3[10..].iter().all(|b| (b & 0x1f) >= 29));
        let bass = analyze(&tone(48000, 1.0, 60.0, 0.8));
        assert!(bass.pwv3[10..].iter().all(|b| b >> 5 <= 1));
        assert!(bass.pwv7[20][0] > bass.pwv7[20][2]);
        let bright = analyze(&tone(48000, 1.0, 8000.0, 0.8));
        assert!(bright.pwv3[10..].iter().all(|b| b >> 5 == 7));
        assert!(bright.pwv7[20][2] > bright.pwv7[20][0]);
    }

    #[test]
    fn chunked_input_measures_like_one_buffer() {
        for (rate, seconds) in [(44100, 1.3), (32000, 0.77), (22050, 0.5)] {
            let s = tone_samples(rate, seconds, 330.0, 0.6);
            let whole = analyze(&metered(rate, 2, &[&s]));
            let pieces: Vec<&[f32]> = s.chunks(2 * 777).collect();
            let chunked = metered(rate, 2, &pieces);
            assert_eq!(chunked.frames as usize, s.len() / 2);
            assert_eq!(analyze(&chunked), whole);
        }
    }

    #[test]
    fn silence_is_zero() {
        let w = analyze(&metered(44100, 1, &[&[0.0; 44100]]));
        assert!(w.pwv3.iter().all(|&b| b == 0xe0));
        assert!(w.pwv7.iter().all(|b| *b == [0, 0, 0]));
        assert_eq!(w.gains, Tuning::default().gain_floor);
    }
}
