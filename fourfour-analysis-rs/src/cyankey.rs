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
const BAND_COMPRESSION: f32 = 0.75;

const SHAATH: [[f64; 12]; 2] = [
    [7.23900502618145, 3.50351166725159, 3.58445177536649, 2.84511816478676, 5.81898892118550, 4.55865057415321, 2.44778850545507, 6.99473192146830, 3.39106613673505, 4.55614256655143, 4.07392666663524, 4.45932757378887],
    [7.00255045060284, 3.14360279015997, 4.35904319714963, 5.40418120718934, 3.67234420879306, 4.08971184917798, 3.90791435991554, 6.19960288562316, 3.63424625625277, 2.87241191079876, 5.35467999794543, 3.83242038595048],
];

pub struct KeyModel {
    pub name: &'static str,
    pub band_compression: f32,
    pub templates: [[f64; NB_FEAT]; 2],
    pub major_bias: f64,
    pub temperature: f64,
    pub cal: [f64; 3],
    pub shaath_tiebreak: bool,
}

pub const CYANBOX: KeyModel = KeyModel {
    name: "cyanbox",
    band_compression: BAND_COMPRESSION,
    templates: TEMPLATES,
    major_bias: MAJOR_BIAS,
    temperature: TEMPERATURE,
    cal: [CAL_A, CAL_B, CAL_C],
    shaath_tiebreak: true,
};

pub const CLEAN_FSL: KeyModel = KeyModel {
    name: "clean-fsl",
    band_compression: 0.75,
    templates: CLEAN_FSL_TEMPLATES,
    major_bias: -1.062633,
    temperature: 1.0,
    cal: [1.14489, 0.0, 0.75727],
    shaath_tiebreak: false,
};

pub const CLEAN_FSL_FMA: KeyModel = KeyModel {
    name: "clean-fsl-fma",
    band_compression: 0.75,
    templates: CLEAN_FSL_FMA_TEMPLATES,
    major_bias: -1.062633,
    temperature: 1.0,
    cal: [1.06842, 0.0, 0.24273],
    shaath_tiebreak: false,
};

pub const SHIPPED: &KeyModel = &CLEAN_FSL_FMA;

const CLEAN_FSL_FMA_TEMPLATES: [[f64; NB_FEAT]; 2] = [
    [
        5.247594, -0.539637, -0.194286, -1.441820, 0.012073, -0.451592, -1.599774, 2.866406, -1.670382, -0.609044, -1.946012, 0.286849,
        8.229498, -2.246683, -1.001166, -4.080767, 4.099496, -0.373058, -3.213686, 4.383755, -3.166371, -1.626284, -3.747497, -0.271996,
        6.899778, -2.617012, 1.567395, -3.551419, 4.550060, -1.773182, -2.476445, 4.417309, -2.559731, -0.660263, -3.736244, 0.902608,
    ],
    [
        5.778344, -1.586362, -1.503399, 0.552986, -1.979291, -0.547380, -1.148601, 2.253464, 0.635049, -2.195802, -0.227042, -0.457681,
        8.119734, -2.797861, -0.238469, 2.477177, -2.239021, -2.011853, -2.285445, 5.143733, -1.808216, -3.655608, 0.244980, -1.130769,
        6.335845, -2.743505, 0.317069, 1.864152, -3.645517, -0.935940, -2.169943, 5.174144, -1.665549, -3.778845, 0.801834, -1.138951,
    ],
];

const CLEAN_FSL_TEMPLATES: [[f64; NB_FEAT]; 2] = [
    [
        3.424011, -0.401199, -0.122028, -1.485625, 0.492735, -0.616705, -1.205425, 2.659577, -1.118791, -0.586604, -1.558231, 0.550910,
        6.898859, -2.224699, -0.940528, -3.118036, 3.397692, -0.302960, -3.185555, 4.003387, -2.653293, -1.718021, -2.774679, 0.158493,
        6.207247, -2.362999, 1.706926, -3.011427, 4.068735, -1.312722, -1.911932, 3.950975, -2.160347, -0.485531, -2.712247, 0.280132,
    ],
    [
        3.816879, -1.157226, -0.708132, 0.363439, -1.630114, 0.120492, -0.892678, 1.900433, 0.488568, -1.810114, 0.002079, -0.488073,
        7.169435, -2.861626, 0.334710, 1.910585, -1.534369, -1.272278, -2.321456, 4.762376, -1.755528, -3.339475, 0.298639, -0.953472,
        5.540063, -3.300362, 0.284672, 1.254265, -2.744346, -0.386343, -2.303418, 4.236576, -1.297062, -3.596907, 0.489643, -0.713528,
    ],
];

pub fn model_by_name(name: &str) -> Option<&'static KeyModel> {
    [&CYANBOX, &CLEAN_FSL, &CLEAN_FSL_FMA].into_iter().find(|m| m.name == name)
}

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

const DECIMATE_LANES: usize = 32;
const DECIMATE_SEGMENT_BLOCKS: usize = 64;

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
    let first_inner = half.div_ceil(d);
    let end_inner = if x.len() > half { ((x.len() - half - 1) / d + 1).min(ny) } else { 0 };
    let mut out = vec![0f32; ny];
    let threads = threads.max(1);
    let per = ny.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in out.chunks_mut(per).enumerate() {
            let (at, h) = (&at, &h);
            s.spawn(move || {
                let base = ci * per;
                let inner_from = base.max(first_inner).min(base + chunk.len());
                let inner_to = (base + chunk.len()).min(end_inner).max(inner_from);
                let inner_end = inner_from + (inner_to - inner_from) / DECIMATE_LANES * DECIMATE_LANES;
                for (i, o) in chunk[..inner_from - base].iter_mut().enumerate() {
                    *o = at(base + i);
                }
                decimate_inner(x, h, d, inner_from, &mut chunk[inner_from - base..inner_end - base]);
                for (i, o) in chunk[inner_end - base..].iter_mut().enumerate() {
                    *o = at(inner_end + i);
                }
            });
        }
    });
    out
}

fn split_phases_of_four(span: &[f32], phases: &mut [f32], phase_len: usize) {
    let (p0, rest) = phases.split_at_mut(phase_len);
    let (p1, rest) = rest.split_at_mut(phase_len);
    let (p2, p3) = rest.split_at_mut(phase_len);
    let rows = span.chunks_exact(4);
    let tail = rows.remainder();
    let full = rows.len().min(phase_len);
    for ((((row, a), b), c), e) in rows.zip(p0.iter_mut()).zip(p1.iter_mut()).zip(p2.iter_mut()).zip(p3.iter_mut()) {
        *a = row[0];
        *b = row[1];
        *c = row[2];
        *e = row[3];
    }
    for (phase, v) in [&mut *p0, &mut *p1, &mut *p2, &mut *p3].into_iter().zip(tail) {
        if full < phase_len {
            phase[full] = *v;
        }
    }
}

fn decimate_inner(x: &[f32], h: &[f32], d: usize, first: usize, out: &mut [f32]) {
    let half = 8 * d;
    let phase_len = DECIMATE_SEGMENT_BLOCKS * DECIMATE_LANES + (h.len() - 1) / d;
    let mut phases = vec![0f32; d * phase_len];
    let tap_offsets: Vec<usize> = (0..h.len()).map(|tap| (tap % d) * phase_len + tap / d).collect();
    for (segment, out) in out.chunks_mut(DECIMATE_SEGMENT_BLOCKS * DECIMATE_LANES).enumerate() {
        let k0 = first + segment * DECIMATE_SEGMENT_BLOCKS * DECIMATE_LANES;
        let from = k0 * d - half;
        let span = &x[from..(from + phase_len * d).min(x.len())];
        if d == 4 {
            split_phases_of_four(span, &mut phases, phase_len);
        } else {
            for (p, phase) in phases.chunks_exact_mut(phase_len).enumerate() {
                for (dst, src) in phase.iter_mut().zip(span[p..].iter().step_by(d)) {
                    *dst = *src;
                }
            }
        }
        for (block, out) in out.chunks_exact_mut(DECIMATE_LANES).enumerate() {
            let mut acc = [0f32; DECIMATE_LANES];
            for (&tap_offset, &hv) in tap_offsets.iter().zip(h) {
                let at = tap_offset + block * DECIMATE_LANES;
                let src: &[f32; DECIMATE_LANES] = phases[at..at + DECIMATE_LANES].try_into().unwrap();
                for j in 0..DECIMATE_LANES {
                    acc[j] += hv * src[j];
                }
            }
            out.copy_from_slice(&acc);
        }
    }
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
        let from = (-start).clamp(0, N_FFT as i64) as usize;
        let to = (self.x.len() as i64 - start).clamp(from as i64, N_FFT as i64) as usize;
        buf[..from].fill(Complex { re: 0.0, im: 0.0 });
        buf[to..].fill(Complex { re: 0.0, im: 0.0 });
        let first = (start + from as i64) as usize;
        for ((c, x), w) in buf[from..to].iter_mut().zip(&self.x[first..first + (to - from)]).zip(&self.win[from..to]) {
            *c = Complex { re: x * w, im: 0.0 };
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

fn raw_bands(stft: &Stft, nb_frames: usize, k: &Kernel, threads: usize) -> Vec<f32> {
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
    rows
}

fn whiten(mut rows: Vec<f32>) -> Vec<f32> {
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

fn compress(mut rows: Vec<f32>, exponent: f32) -> Vec<f32> {
    for v in rows.iter_mut() {
        *v = v.powf(exponent);
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

fn scores(model: &KeyModel, x: &[f64; NB_FEAT]) -> [f64; 24] {
    let mut s = [0f64; 24];
    for mode in 0..2 {
        for k in 0..12 {
            let mut acc = if mode == 0 { model.major_bias } else { 0.0 };
            for v in 0..3 {
                for i in 0..12 {
                    acc += model.templates[mode][12 * v + i] * x[12 * v + (i + k) % 12];
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

fn best_key(model: &KeyModel, x: &[f64; NB_FEAT], s: &[f64; 24]) -> usize {
    let mut best = 0;
    for k in 1..24 {
        let d = s[k] - s[best];
        if d > 1e-9 {
            best = k;
        } else if model.shaath_tiebreak && d >= -1e-9 {
            let (sk, sb) = (shaath_cosine(x, k), shaath_cosine(x, best));
            if sk > sb + 1e-12 || ((sk - sb).abs() <= 1e-12 && k >= 12 && best < 12) {
                best = k;
            }
        }
    }
    best
}

pub struct Bands {
    pub rows: Vec<f32>,
    pub nb_frames: usize,
    pub tuning: f64,
    pub fs: f64,
}

pub fn semitone_bands(mono: &[f32], rate: u32, threads: usize) -> Option<Bands> {
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
    let rows = raw_bands(&stft, nb_frames, &k, threads);
    Some(Bands { rows, nb_frames, tuning, fs })
}

pub fn detect(mono: &[f32], rate: u32, threads: usize) -> Option<CyanKey> {
    detect_with(SHIPPED, mono, rate, threads)
}

pub fn detect_with(model: &KeyModel, mono: &[f32], rate: u32, threads: usize) -> Option<CyanKey> {
    let Bands { rows, nb_frames, tuning, fs } = semitone_bands(mono, rate, threads)?;
    let rows = compress(whiten(rows), model.band_compression);

    let xs = chroma(&rows, 0, nb_frames);
    if xs.iter().sum::<f64>() <= 0.0 {
        return None;
    }
    let sc = scores(model, &xs);
    let best = best_key(model, &xs, &sc);
    let sum: f64 = sc.iter().map(|s| (model.temperature * (s - sc[best])).exp()).sum();
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
        let ss = scores(model, &xseg);
        if best_key(model, &xseg, &ss) == best {
            nb_agree += 1;
        }
        nb_seg += 1;
    }
    let agreement = if nb_seg > 0 { nb_agree as f64 / nb_seg as f64 } else { 0.0 };
    let confidence = 1.0 / (1.0 + (-(model.cal[0] * (p / (1.0 - p)).ln() + model.cal[1] * agreement + model.cal[2])).exp());
    Some(CyanKey { class: best, confidence, tuning_cents: 100.0 * tuning })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decimate_reference(x: &[f32], d: usize) -> Vec<f32> {
        let h = decimation_taps(d);
        let half = 8 * d;
        (0..x.len().div_ceil(d))
            .map(|k| {
                let mut acc = 0f32;
                for (i, hv) in h.iter().enumerate() {
                    let j = (k * d + i) as i64 - half as i64;
                    if j >= 0 && (j as usize) < x.len() {
                        acc += hv * x[j as usize];
                    }
                }
                acc
            })
            .collect()
    }

    #[test]
    fn decimation_matches_the_scalar_filter_bit_for_bit() {
        let mut seed = 0x9e3779b9u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
        };
        for d in [2, 4, 7] {
            for len in [0, 1, 5, 33, 64 * d, 400 * d + 3, 12_345, 40_000, 40_001, 40_002, 40_003] {
                let x: Vec<f32> = (0..len).map(|_| noise()).collect();
                for threads in [1, 3] {
                    let fast = decimate(&x, d, threads);
                    let slow = decimate_reference(&x, d);
                    assert_eq!(fast.len(), slow.len());
                    assert!(fast.iter().zip(&slow).all(|(a, b)| a.to_bits() == b.to_bits()), "d {d} len {len} threads {threads}");
                }
            }
        }
    }

    #[test]
    fn stft_frames_match_the_bounds_checked_fill_at_both_ends() {
        let x: Vec<f32> = (0..3 * HOP + 777).map(|i| ((i * 31 % 101) as f32 - 50.0) / 50.0).collect();
        let stft = Stft::new(&x);
        let mut buf = vec![Complex { re: 0f32, im: 0f32 }; N_FFT];
        let mut mag = vec![0f32; N_FFT / 2 + 1];
        for f in 0..x.len() / HOP + 1 {
            stft.magnitudes(f, N_FFT / 2, &mut buf, &mut mag);
            let start = f as i64 * HOP as i64 - (N_FFT / 2) as i64;
            let mut expected: Vec<Complex<f32>> = (0..N_FFT)
                .map(|i| {
                    let j = start + i as i64;
                    let v = if j >= 0 && (j as usize) < x.len() { x[j as usize] * stft.win[i] } else { 0.0 };
                    Complex { re: v, im: 0.0 }
                })
                .collect();
            stft.fft.process(&mut expected);
            assert!((0..N_FFT / 2).all(|b| mag[b].to_bits() == expected[b].norm().to_bits()), "frame {f}");
        }
    }

    #[test]
    fn shipped_model_is_clean_fsl_fma_and_all_models_resolve() {
        assert_eq!(SHIPPED.name, "clean-fsl-fma");
        assert_eq!(model_by_name("clean-fsl-fma").map(|m| m.name), Some("clean-fsl-fma"));
        assert_eq!(model_by_name("cyanbox").map(|m| m.name), Some("cyanbox"));
        assert_eq!(model_by_name("clean-fsl").map(|m| m.name), Some("clean-fsl"));
        assert!(model_by_name("other").is_none());
    }

    #[test]
    fn models_agree_on_a_plain_a_minor_chord() {
        let rate = 44100;
        let tone = |f: f64, i: usize| (2.0 * PI * f * i as f64 / rate as f64).sin() as f32;
        let mono: Vec<f32> = (0..rate as usize * 12).map(|i| 0.3 * (tone(220.0, i) + tone(261.63, i) + tone(329.63, i) + tone(110.0, i))).collect();
        let a = detect_with(&CYANBOX, &mono, rate, 2).unwrap();
        let c = detect_with(&CLEAN_FSL, &mono, rate, 2).unwrap();
        let b = detect(&mono, rate, 2).unwrap();
        assert_eq!(a.class, 21);
        assert_eq!(c.class, 21);
        assert_eq!(b.class, 21);
        assert!(b.confidence > 0.0 && b.confidence < 1.0);
    }
}
