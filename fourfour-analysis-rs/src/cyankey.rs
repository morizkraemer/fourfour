// Port of cyanbox src/analysis/key.c (tuning-corrected, whitened semitone spectrum, three chroma
// vectors, fitted templates). See NOTICE for the original licence text.
// Copyright (c) 2026, Lynne. BSD-2-Clause.

use std::f64::consts::PI;

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const KEY_RATE: f64 = 11025.0;
const N_FFT: usize = 8192;
const HOP: usize = 4096;
const MIDI_LO: i32 = 24;
const NB_BANDS: usize = 72;
const WHITE: i64 = 6;
const NB_FEAT: usize = 36;
const PEAK_LO: f64 = 150.0;
const PEAK_HI: f64 = 4000.0;
const PEAK_FLOOR: f32 = 1e-2;
const TUNE_STEP: usize = N_FFT / HOP;
const SEG_SECONDS: f64 = 20.0;

const SHAATH: [[f64; 12]; 2] = [
    [7.23900502618145, 3.50351166725159, 3.58445177536649, 2.84511816478676, 5.81898892118550, 4.55865057415321, 2.44778850545507, 6.99473192146830, 3.39106613673505, 4.55614256655143, 4.07392666663524, 4.45932757378887],
    [7.00255045060284, 3.14360279015997, 4.35904319714963, 5.40418120718934, 3.67234420879306, 4.08971184917798, 3.90791435991554, 6.19960288562316, 3.63424625625277, 2.87241191079876, 5.35467999794543, 3.83242038595048],
];

const TEMPLATES: [[f64; NB_FEAT]; 2] = [
    [
        3.831086, 0.422911, -0.541941, -0.268662, -0.156559, -0.494596, -1.285167, 2.300662, -1.089226, -0.421346, -1.739469, -0.762827,
        9.146794, -4.450626, 2.018196, -7.888354, 7.598707, -1.006002, -4.606749, 6.932495, -5.131950, 0.977236, -2.524926, 0.054478,
        8.470257, -6.448374, 2.721740, -4.680061, 7.415636, -0.726577, -6.426598, 8.398842, -8.124914, -0.729397, -4.172607, 0.629826,
    ],
    [
        4.320080, 0.815372, -1.393019, 0.151058, -1.310203, -0.692815, -0.389036, 1.268844, -0.019809, -3.557954, 1.532833, -0.520218,
        8.244831, -5.135689, 0.128518, 1.839275, 1.997730, -2.337059, -4.984217, 8.378914, -3.717863, -4.856572, 2.971073, -3.648241,
        9.820503, -5.160344, 0.129472, 6.367447, -2.777826, 0.897393, -3.997295, 10.009723, -5.713693, -3.671759, 2.989522, -5.220917,
    ],
];
const MAJOR_BIAS: f64 = 0.442;
const TEMPERATURE: f64 = 0.9349;
const CAL_A: f64 = 0.62796;
const CAL_B: f64 = 0.79616;
const CAL_C: f64 = -0.33589;

#[derive(Clone, Debug)]
pub struct CyanKey {
    pub class: usize,
    pub confidence: f64,
    pub tuning_cents: f64,
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, q) = (1.0, 1.0, x * x / 4.0);
    for k in 1..500 {
        term *= q / ((k * k) as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

fn decimation_taps(d: usize) -> Vec<f32> {
    let half = 8 * d;
    let taps = 2 * half + 1;
    let i0b = bessel_i0(6.0);
    let mut h = vec![0f32; taps];
    let mut sum = 0.0;
    for (i, hv) in h.iter_mut().enumerate() {
        let m = i as f64 - half as f64;
        let r = m / half as f64;
        let s = if m == 0.0 { 1.0 } else { (PI * m / d as f64).sin() / (PI * m / d as f64) };
        *hv = (s * bessel_i0(6.0 * (1.0 - r * r).sqrt()) / i0b) as f32;
        sum += *hv as f64;
    }
    for hv in h.iter_mut() {
        *hv = (*hv as f64 / sum) as f32;
    }
    h
}

fn decimate(x: &[f32], d: usize, threads: usize) -> Vec<f32> {
    let h = decimation_taps(d);
    let half = 8 * d;
    let ny = x.len().div_ceil(d);
    let at = |k: usize| -> f32 {
        let c = k * d;
        let mut acc = 0f32;
        if c >= half && c - half + h.len() <= x.len() {
            let p = &x[c - half..c - half + h.len()];
            for (a, b) in h.iter().zip(p) {
                acc += a * b;
            }
        } else {
            for (i, hv) in h.iter().enumerate() {
                let j = (c + i) as i64 - half as i64;
                if j >= 0 && (j as usize) < x.len() {
                    acc += hv * x[j as usize];
                }
            }
        }
        acc
    };
    let mut out = vec![0f32; ny];
    let threads = threads.max(1);
    let per = ny.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in out.chunks_mut(per).enumerate() {
            let at = &at;
            s.spawn(move || {
                for (i, o) in chunk.iter_mut().enumerate() {
                    *o = at(ci * per + i);
                }
            });
        }
    });
    out
}

struct Stft<'a> {
    x: &'a [f32],
    win: Vec<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
}

impl<'a> Stft<'a> {
    fn new(x: &'a [f32]) -> Self {
        let win = (0..N_FFT).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / N_FFT as f64).cos()) as f32).collect();
        Stft { x, win, fft: FftPlanner::<f32>::new().plan_fft_forward(N_FFT) }
    }

    fn magnitudes(&self, f: usize, nb: usize, buf: &mut [Complex<f32>], mag: &mut [f32]) {
        let start = f as i64 * HOP as i64 - (N_FFT / 2) as i64;
        for (i, c) in buf.iter_mut().enumerate() {
            let j = start + i as i64;
            let v = if j >= 0 && (j as usize) < self.x.len() { self.x[j as usize] * self.win[i] } else { 0.0 };
            *c = Complex { re: v, im: 0.0 };
        }
        self.fft.process(buf);
        for b in 0..nb {
            mag[b] = buf[b].norm();
        }
    }
}

fn estimate_tuning(stft: &Stft, nb_frames: usize, fs: f64) -> f64 {
    let df = fs / N_FFT as f64;
    let lo = ((PEAK_LO / df).ceil() as usize).max(1);
    let hi = (((PEAK_HI).min(0.45 * fs) / df) as usize).min(N_FFT / 2 - 1);
    if hi <= lo {
        return 0.0;
    }
    let (mut re, mut im) = (0f64, 0f64);
    let mut buf = vec![Complex { re: 0f32, im: 0f32 }; N_FFT];
    let mut mag = vec![0f32; N_FFT / 2 + 1];
    let mut f = 0;
    while f < nb_frames {
        stft.magnitudes(f, hi + 2, &mut buf, &mut mag);
        f += TUNE_STEP;
        let top = mag[lo..=hi].iter().cloned().fold(0f32, f32::max);
        if top <= 0.0 {
            continue;
        }
        let floor_mag = top * PEAK_FLOOR;
        for b in lo..=hi {
            let m = mag[b];
            if !(m > floor_mag && m > mag[b - 1] && m >= mag[b + 1]) {
                continue;
            }
            let a = (mag[b - 1] + 1e-9).ln();
            let c = (mag[b + 1] + 1e-9).ln();
            let l = m.ln();
            let off = (0.5 * (a - c) / (a - 2.0 * l + c - 1e-12)).clamp(-0.5, 0.5);
            let peak = l - 0.25 * (a - c) * off;
            let dev = 12.0 * ((b as f32 + off) * df as f32 / 440.0).log2();
            let w = (2.0 * peak).exp();
            let (sn, cs) = (2.0 * std::f32::consts::PI * (dev - dev.round_ties_even())).sin_cos();
            re += (w * cs) as f64;
            im += (w * sn) as f64;
        }
    }
    if re == 0.0 && im == 0.0 {
        return 0.0;
    }
    im.atan2(re) / (2.0 * PI)
}

struct Kernel {
    start: [usize; NB_BANDS],
    len: [usize; NB_BANDS],
    w: Vec<f32>,
    nb_bins: usize,
}

fn kernel(fs: f64, tuning: f64) -> Kernel {
    let mut k = Kernel { start: [0; NB_BANDS], len: [0; NB_BANDS], w: Vec::new(), nb_bins: 0 };
    let mut centre = [0f64; NB_BANDS];
    let mut width = [0f64; NB_BANDS];
    for m in 0..NB_BANDS {
        let fc = 440.0 * 2f64.powf(((MIDI_LO + m as i32 - 69) as f64 + tuning) / 12.0);
        centre[m] = fc * N_FFT as f64 / fs;
        width[m] = (centre[m] * 0.8 * (2f64.powf(1.0 / 12.0) - 1.0)).max(2.0);
        let b0 = centre[m] - width[m] / 2.0;
        let first = b0.ceil() as i64;
        let mut last = (b0 + width[m]).floor() as i64;
        if last >= (N_FFT / 2) as i64 {
            last = first - 1;
        }
        k.start[m] = first.max(0) as usize;
        k.len[m] = (last - first + 1).max(0) as usize;
        if k.len[m] > 0 {
            k.nb_bins = k.nb_bins.max(last as usize + 1);
        }
    }
    for m in 0..NB_BANDS {
        let b0 = centre[m] - width[m] / 2.0;
        let mut ws = Vec::with_capacity(k.len[m]);
        let mut sum = 0.0;
        for i in 0..k.len[m] {
            let w = (1.0 - (2.0 * PI * ((k.start[m] + i) as f64 - b0) / width[m]).cos()) as f32;
            sum += w as f64;
            ws.push(w);
        }
        for w in ws {
            k.w.push((w as f64 / sum * width[m]) as f32);
        }
    }
    k
}

fn bands(stft: &Stft, nb_frames: usize, k: &Kernel, threads: usize) -> Vec<f32> {
    let mut rows = vec![0f32; nb_frames * NB_BANDS];
    let per = nb_frames.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in rows.chunks_mut(per * NB_BANDS).enumerate() {
            s.spawn(move || {
                let mut buf = vec![Complex { re: 0f32, im: 0f32 }; N_FFT];
                let mut mag = vec![0f32; N_FFT / 2 + 1];
                for (fi, row) in chunk.chunks_mut(NB_BANDS).enumerate() {
                    stft.magnitudes(ci * per + fi, k.nb_bins, &mut buf, &mut mag);
                    let mut off = 0;
                    for m in 0..NB_BANDS {
                        let mut acc = 0f32;
                        for i in 0..k.len[m] {
                            acc += k.w[off + i] * mag[k.start[m] + i];
                        }
                        off += k.len[m];
                        row[m] = acc;
                    }
                }
            });
        }
    });
    let top = rows.iter().cloned().fold(0f32, f32::max);
    let floor_mag = 1e-6 * top + 1e-12;
    for row in rows.chunks_mut(NB_BANDS) {
        let mut env = [0f32; NB_BANDS];
        for m in 0..NB_BANDS {
            let mut acc = 0f32;
            for j in m as i64 - WHITE..=m as i64 + WHITE {
                acc += row[j.clamp(0, NB_BANDS as i64 - 1) as usize];
            }
            env[m] = acc / (2 * WHITE + 1) as f32;
        }
        for m in 0..NB_BANDS {
            row[m] /= env[m] + floor_mag;
        }
    }
    rows
}

fn chroma(rows: &[f32], f0: usize, f1: usize) -> [f64; NB_FEAT] {
    const RANGE: [[usize; 2]; 3] = [[12, 24], [24, 48], [48, 72]];
    let mut out = [0f64; NB_FEAT];
    for f in f0..f1 {
        let row = &rows[f * NB_BANDS..(f + 1) * NB_BANDS];
        for (v, r) in RANGE.iter().enumerate() {
            for m in r[0]..r[1] {
                out[12 * v + (MIDI_LO as usize + m) % 12] += row[m] as f64;
            }
        }
    }
    for v in 0..3 {
        let norm = out[12 * v..12 * v + 12].iter().map(|x| x * x).sum::<f64>().sqrt();
        for i in 0..12 {
            out[12 * v + i] = if norm > 0.0 { out[12 * v + i] / norm } else { 0.0 };
        }
    }
    out
}

fn scores(x: &[f64; NB_FEAT]) -> [f64; 24] {
    let mut s = [0f64; 24];
    for mode in 0..2 {
        for k in 0..12 {
            let mut acc = if mode == 0 { MAJOR_BIAS } else { 0.0 };
            for v in 0..3 {
                for i in 0..12 {
                    acc += TEMPLATES[mode][12 * v + i] * x[12 * v + (i + k) % 12];
                }
            }
            s[12 * mode + k] = acc;
        }
    }
    s
}

fn shaath_cosine(x: &[f64; NB_FEAT], key: usize) -> f64 {
    let p = &SHAATH[key / 12];
    let mut c = [0f64; 12];
    for i in 0..12 {
        c[i] = x[i] + x[12 + i] + x[24 + i];
    }
    let (mut dot, mut nc, mut np) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let v = c[(i + key) % 12];
        dot += v * p[i];
        nc += v * v;
        np += p[i] * p[i];
    }
    if nc > 0.0 { dot / (nc * np).sqrt() } else { 0.0 }
}

fn best_key(x: &[f64; NB_FEAT], s: &[f64; 24]) -> usize {
    let mut best = 0;
    for k in 1..24 {
        let d = s[k] - s[best];
        if d > 1e-9 {
            best = k;
        } else if d >= -1e-9 {
            let (sk, sb) = (shaath_cosine(x, k), shaath_cosine(x, best));
            if sk > sb + 1e-12 || ((sk - sb).abs() <= 1e-12 && k >= 12 && best < 12) {
                best = k;
            }
        }
    }
    best
}

pub fn detect(mono: &[f32], rate: u32, threads: usize) -> Option<CyanKey> {
    if rate == 0 || mono.is_empty() {
        return None;
    }
    let d = ((rate as f64 / KEY_RATE).round() as usize).max(1);
    let decimated;
    let x: &[f32] = if d > 1 {
        decimated = decimate(mono, d, threads);
        &decimated
    } else {
        mono
    };
    let fs = rate as f64 / d as f64;
    let nb_frames = x.len() / HOP + 1;
    let stft = Stft::new(x);
    let tuning = estimate_tuning(&stft, nb_frames, fs);
    let k = kernel(fs, tuning);
    let rows = bands(&stft, nb_frames, &k, threads);

    let xs = chroma(&rows, 0, nb_frames);
    if xs.iter().sum::<f64>() <= 0.0 {
        return None;
    }
    let sc = scores(&xs);
    let best = best_key(&xs, &sc);
    let sum: f64 = sc.iter().map(|s| (TEMPERATURE * (s - sc[best])).exp()).sum();
    let p = (1.0 / sum).clamp(1e-6, 1.0 - 1e-6);

    let seg = ((SEG_SECONDS * fs / HOP as f64).round() as usize).max(1);
    let (mut nb_seg, mut nb_agree) = (0, 0);
    let mut f0 = 0;
    while f0 < nb_frames {
        let f1 = (f0 + seg).min(nb_frames);
        if f0 > 0 && f1 - f0 < seg / 2 {
            break;
        }
        let xseg = chroma(&rows, f0, f1);
        f0 += seg;
        if xseg.iter().sum::<f64>() <= 0.0 {
            continue;
        }
        let ss = scores(&xseg);
        if best_key(&xseg, &ss) == best {
            nb_agree += 1;
        }
        nb_seg += 1;
    }
    let agreement = if nb_seg > 0 { nb_agree as f64 / nb_seg as f64 } else { 0.0 };
    let confidence = 1.0 / (1.0 + (-(CAL_A * (p / (1.0 - p)).ln() + CAL_B * agreement + CAL_C)).exp());
    Some(CyanKey { class: best, confidence, tuning_cents: 100.0 * tuning })
}
