use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::f64::consts::PI;

pub const TARGET_SR: u32 = 12_000;
pub const SEGMENT: usize = 80;
pub const DETAIL_PER_SECOND: f64 = 150.0;
pub const OVERVIEW_ENTRIES: usize = 1200;
pub const PWAV_ENTRIES: usize = 400;

const LOW_CUTOFF_HZ: f64 = 130.0;
const MID_CUTOFF_HZ: f64 = 2500.0;
const FILTER_ORDER: usize = 4;
const GAINS: [f64; 3] = [140.0, 120.0, 70.0];
const POWER: f64 = 0.5;
const MIX_FACTOR: f64 = 0.1;
const PEAK_HOLD: usize = 3;
const CHANNEL_MAX: f64 = 127.0;

pub struct Waveforms {
    pub pwav: [u8; PWAV_ENTRIES],
    pub detail: Vec<[u8; 3]>,
    pub overview: Vec<[u8; 3]>,
}

struct Column {
    peak: f32,
    rgb: [u8; 3],
}

#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
}

fn butter_bandpass(lo: f64, hi: f64, order: usize) -> Vec<Biquad> {
    let fs = 2.0;
    let warp = |w: f64| 2.0 * fs * (PI * w / fs).tan();
    let (wl, wh) = (warp(lo), warp(hi));
    let bandwidth = wh - wl;
    let centre = (wl * wh).sqrt();
    let mut sections = Vec::new();
    for m in (-(order as i64) + 1..order as i64).step_by(2) {
        let prototype = -Complex::new(0.0, PI * m as f64 / (2.0 * order as f64)).exp();
        let scaled = prototype * (bandwidth / 2.0);
        let root = (scaled * scaled - Complex::new(centre * centre, 0.0)).sqrt();
        for analog in [scaled + root, scaled - root] {
            if analog.im <= 0.0 {
                continue;
            }
            let z = (Complex::new(2.0 * fs, 0.0) + analog) / (Complex::new(2.0 * fs, 0.0) - analog);
            sections.push(Biquad { b: [1.0, 0.0, -1.0], a: [-2.0 * z.re, z.norm_sqr()] });
        }
    }
    assert_eq!(sections.len(), order, "bandpass pole pairing");
    let w0 = 2.0 * (centre / (2.0 * fs)).atan();
    let z1 = Complex::new(w0.cos(), -w0.sin());
    let z2 = z1 * z1;
    let response: Complex<f64> = sections.iter().fold(Complex::new(1.0, 0.0), |acc, s| {
        acc * (Complex::new(s.b[0], 0.0) + z1 * s.b[1] + z2 * s.b[2]) / (Complex::new(1.0, 0.0) + z1 * s.a[0] + z2 * s.a[1])
    });
    for b in sections[0].b.iter_mut() {
        *b /= response.norm();
    }
    sections
}

fn band_segment_rms(audio: &[f32], sections: &[Biquad], segments: usize) -> Vec<f64> {
    let mut state = vec![[0.0f64; 2]; sections.len()];
    let mut sums = vec![0.0f64; segments];
    for (i, &x) in audio[..segments * SEGMENT].iter().enumerate() {
        let mut v = x as f64;
        for (s, st) in sections.iter().zip(state.iter_mut()) {
            let y = s.b[0] * v + st[0];
            st[0] = s.b[1] * v - s.a[0] * y + st[1];
            st[1] = s.b[2] * v - s.a[1] * y;
            v = y;
        }
        sums[i / SEGMENT] += v * v;
    }
    sums.into_iter().map(|s| (s / SEGMENT as f64).sqrt()).collect()
}

fn columns(audio12k: &[f32]) -> Vec<Column> {
    let segments = audio12k.len() / SEGMENT;
    if segments == 0 {
        return Vec::new();
    }
    let nyquist = TARGET_SR as f64 / 2.0;
    let edges = [
        (0.0, LOW_CUTOFF_HZ / nyquist),
        (LOW_CUTOFF_HZ / nyquist, MID_CUTOFF_HZ / nyquist),
        (MID_CUTOFF_HZ / nyquist, 1.0),
    ];
    let bands: Vec<Vec<f64>> = edges
        .iter()
        .map(|&(lo, hi)| band_segment_rms(audio12k, &butter_bandpass(lo.max(0.001), hi.min(0.999), FILTER_ORDER), segments))
        .collect();
    let mut previous = [0.0f64; 3];
    (0..segments)
        .map(|i| {
            let window = i..(i + PEAK_HOLD + 1).min(segments);
            let mut rgb = [0u8; 3];
            for c in 0..3 {
                let held = bands[c][window.clone()].iter().cloned().fold(0.0f64, f64::max);
                let raw = GAINS[c] * held.powf(POWER);
                let smoothed = previous[c] * MIX_FACTOR + raw * (1.0 - MIX_FACTOR);
                previous[c] = smoothed;
                rgb[c] = smoothed.round_ties_even().clamp(0.0, CHANNEL_MAX) as u8;
            }
            let segment = &audio12k[i * SEGMENT..(i + 1) * SEGMENT];
            Column { peak: segment.iter().cloned().fold(f32::MIN, f32::max), rgb }
        })
        .collect()
}

fn preview(columns: &[Column]) -> [u8; PWAV_ENTRIES] {
    let mut out = [0u8; PWAV_ENTRIES];
    let n = columns.len();
    if n == 0 {
        return out;
    }
    for (i, o) in out.iter_mut().enumerate() {
        let (start, end) = (i * n / PWAV_ENTRIES, (i + 1) * n / PWAV_ENTRIES);
        let bin = if end > start { &columns[start..end] } else { &columns[start..start + 1] };
        let mean_peak = bin.iter().map(|c| c.peak.abs() as f64).sum::<f64>() / bin.len() as f64;
        let height = ((mean_peak * 31.0).round_ties_even() as u32).min(31) as u8;
        let channel_max = |c: usize| bin.iter().map(|col| col.rgb[c] as u32).max().unwrap_or(0);
        let total = channel_max(0) + channel_max(1) + channel_max(2);
        let whiteness = if total > 0 { ((channel_max(2) as f64 / total as f64 * 7.0).round_ties_even() as u8).min(7) } else { 0 };
        *o = (whiteness << 5) | height;
    }
    out
}

fn overview(columns: &[Column]) -> Vec<[u8; 3]> {
    let n = columns.len();
    if n <= OVERVIEW_ENTRIES {
        return columns.iter().map(|c| c.rgb).collect();
    }
    (0..OVERVIEW_ENTRIES)
        .map(|i| {
            let (start, end) = (i * n / OVERVIEW_ENTRIES, (i + 1) * n / OVERVIEW_ENTRIES);
            let bin = if end > start { &columns[start..end] } else { &columns[start..start + 1] };
            let max = |c: usize| bin.iter().map(|col| col.rgb[c]).max().unwrap_or(0);
            [max(0), max(1), max(2)]
        })
        .collect()
}

pub fn compute(audio12k: &[f32]) -> Waveforms {
    let cols = columns(audio12k);
    Waveforms { pwav: preview(&cols), detail: cols.iter().map(|c| c.rgb).collect(), overview: overview(&cols) }
}

pub fn checksum(w: &Waveforms) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    w.pwav.iter().for_each(|b| eat(*b));
    w.detail.iter().chain(&w.overview).flatten().for_each(|b| eat(*b));
    h
}

const PLACEHOLDER_FFT: usize = 2048;

pub fn compute_placeholder(mono: &[f32], sr: u32, threads: usize) -> Waveforms {
    let duration_s = mono.len() as f64 / sr as f64;
    Waveforms {
        pwav: placeholder_preview(mono),
        detail: placeholder_entries(mono, sr, (duration_s * DETAIL_PER_SECOND) as usize, threads),
        overview: placeholder_entries(mono, sr, OVERVIEW_ENTRIES, threads),
    }
}

pub fn placeholder_entries(mono: &[f32], sr: u32, entries: usize, threads: usize) -> Vec<[u8; 3]> {
    let hop = (mono.len() / entries.max(1)).max(1);
    let fft = FftPlanner::<f32>::new().plan_fft_forward(PLACEHOLDER_FFT);
    let bin_hz = sr as f32 / PLACEHOLDER_FFT as f32;
    let bin_of = |hz: f32| (hz / bin_hz).ceil() as usize;
    let (low_lo, low_hi, high_lo) = (bin_of(20.0), bin_of(250.0), bin_of(4000.0));
    let (mid_lo, mid_hi) = (low_hi, high_lo);
    let nbins = PLACEHOLDER_FFT / 2 + 1;
    let count = entries.min(mono.len() / hop);

    let mut raw = vec![(0f32, 0f32, 0f32); count];
    let per = count.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (ci, chunk_out) in raw.chunks_mut(per).enumerate() {
            let fft = &fft;
            s.spawn(move || {
                let mut buf = vec![Complex { re: 0f32, im: 0f32 }; PLACEHOLDER_FFT];
                for (k, out) in chunk_out.iter_mut().enumerate() {
                    let i = ci * per + k;
                    let chunk = &mono[i * hop..(i + 1) * hop];
                    for (j, c) in buf.iter_mut().enumerate() {
                        *c = Complex { re: chunk.get(j).copied().unwrap_or(0.0), im: 0.0 };
                    }
                    fft.process(&mut buf);
                    let mean = |lo: usize, hi: usize| {
                        let hi = hi.min(nbins);
                        if hi <= lo { 0.0 } else { buf[lo..hi].iter().map(|c| c.norm()).sum::<f32>() / (hi - lo) as f32 }
                    };
                    *out = (mean(low_lo, low_hi), mean(mid_lo, mid_hi), mean(high_lo, nbins));
                }
            });
        }
    });
    let (mut max_l, mut max_m, mut max_h) = (0f32, 0f32, 0f32);
    for (l, m, h) in &raw {
        max_l = max_l.max(*l);
        max_m = max_m.max(*m);
        max_h = max_h.max(*h);
    }
    let scale = |v: f32, max: f32, top: f32| if max > 0.0 { ((v / max) * top) as u8 } else { 0 };
    let mut out: Vec<[u8; 3]> = raw.into_iter().map(|(l, m, h)| [scale(l, max_l, 96.0), scale(m, max_m, 64.0), scale(h, max_h, 48.0)]).collect();
    out.resize(entries, [0, 0, 0]);
    out
}

pub fn placeholder_preview(mono: &[f32]) -> [u8; PWAV_ENTRIES] {
    let chunk = (mono.len() / PWAV_ENTRIES).max(1);
    let mut rms = [0f32; PWAV_ENTRIES];
    for (i, r) in rms.iter_mut().enumerate() {
        let start = i * chunk;
        if start >= mono.len() {
            break;
        }
        let end = (start + chunk).min(mono.len());
        let c = &mono[start..end];
        *r = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
    }
    let max = rms.iter().cloned().fold(0f32, f32::max);
    let mut out = [0u8; PWAV_ENTRIES];
    if max > 0.0 {
        for (o, r) in out.iter_mut().zip(rms) {
            let n = r / max;
            *o = (((n * 7.0) as u8) << 5) | ((n * 31.0) as u8 & 0x1F);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, seconds: f64) -> Vec<f32> {
        (0..(TARGET_SR as f64 * seconds) as usize).map(|i| (0.5 * (2.0 * PI * hz * i as f64 / TARGET_SR as f64).sin()) as f32).collect()
    }

    #[test]
    fn bandpass_has_unit_gain_at_centre_and_rejects_far_bands() {
        let sections = butter_bandpass(130.0 / 6000.0, 2500.0 / 6000.0, FILTER_ORDER);
        let gain = |hz: f64| {
            let audio = sine(hz, 2.0);
            let rms = band_segment_rms(&audio, &sections, audio.len() / SEGMENT);
            let tail = &rms[rms.len() / 2..];
            tail.iter().sum::<f64>() / tail.len() as f64 / (0.5 / 2f64.sqrt())
        };
        assert!((gain(570.0) - 1.0).abs() < 0.05, "centre {}", gain(570.0));
        assert!(gain(20.0) < 0.05);
        assert!(gain(5900.0) < 0.1);
    }

    #[test]
    fn bass_tone_lights_low_channel_only() {
        let w = compute(&sine(60.0, 4.0));
        let mid = w.detail[w.detail.len() / 2];
        assert!(mid[0] > 20, "{mid:?}");
        assert!(mid[1] < mid[0] / 4 && mid[2] < mid[0] / 4, "{mid:?}");
        assert_eq!(w.detail.len(), 4 * 150);
        assert_eq!(w.overview.len(), 600);
    }

    #[test]
    fn silence_is_all_zero() {
        let w = compute(&vec![0.0; 12_000]);
        assert!(w.pwav.iter().all(|b| *b == 0));
        assert!(w.detail.iter().all(|c| *c == [0, 0, 0]));
    }

    #[test]
    fn long_tracks_get_1200_overview_entries() {
        assert_eq!(compute(&sine(100.0, 20.0)).overview.len(), OVERVIEW_ENTRIES);
    }
}
