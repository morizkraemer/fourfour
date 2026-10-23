use std::f32::consts::PI;

use rustfft::{num_complex::Complex, FftPlanner};

pub const MIDI_LO: usize = 24;
pub const BINS_PER_SEMITONE: usize = 3;
pub const SEMITONES: usize = 73;
pub const NBINS: usize = SEMITONES * BINS_PER_SEMITONE;
pub const ANALYSIS_RATE: u32 = 11025;
const FFT_SIZE: usize = 8192;
const HOP: usize = 2048;
const DECIMATION: usize = 4;
const FIR_HALF_LENGTH: usize = 64;

pub struct Spec {
    pub frames: usize,
    pub data: Vec<f32>,
}

impl Spec {
    pub fn frame(&self, t: usize) -> &[f32] {
        &self.data[t * NBINS..(t + 1) * NBINS]
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.data.len() * 4);
        out.extend_from_slice(&(self.frames as u32).to_le_bytes());
        for v in &self.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Spec {
        let frames = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let data = bytes[4..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        Spec { frames, data }
    }
}

pub fn bin_frequency(bin: usize) -> f32 {
    let midi = MIDI_LO as f32 + (bin as f32 - 1.0) / BINS_PER_SEMITONE as f32;
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

fn decimate(samples_44k: &[f32]) -> Vec<f32> {
    let cutoff = 4900.0 / 44100.0;
    let taps: Vec<f32> = (0..=2 * FIR_HALF_LENGTH)
        .map(|i| {
            let n = i as f32 - FIR_HALF_LENGTH as f32;
            let sinc = if n == 0.0 { 2.0 * cutoff } else { (2.0 * PI * cutoff * n).sin() / (PI * n) };
            let window = 0.5 + 0.5 * (PI * n / (FIR_HALF_LENGTH as f32 + 1.0)).cos();
            sinc * window
        })
        .collect();
    let out_len = samples_44k.len() / DECIMATION;
    let mut padded = vec![0f32; FIR_HALF_LENGTH];
    padded.extend_from_slice(samples_44k);
    padded.extend(std::iter::repeat(0f32).take(FIR_HALF_LENGTH + DECIMATION));
    (0..out_len)
        .map(|i| {
            let window = &padded[i * DECIMATION..i * DECIMATION + taps.len()];
            window.iter().zip(&taps).map(|(x, h)| x * h).sum()
        })
        .collect()
}

pub fn analyze(samples_44k: &[f32]) -> Spec {
    let mono = decimate(samples_44k);
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / FFT_SIZE as f32).cos()).collect();
    let bin_hz = ANALYSIS_RATE as f32 / FFT_SIZE as f32;
    let half_semitone_ratio = 2f32.powf(1.0 / 36.0);
    let ranges: Vec<(f32, f32)> = (0..NBINS)
        .map(|b| {
            let f = bin_frequency(b);
            (f / half_semitone_ratio / bin_hz, f * half_semitone_ratio / bin_hz)
        })
        .collect();

    let frames = if mono.len() >= FFT_SIZE { (mono.len() - FFT_SIZE) / HOP + 1 } else { 0 };
    let mut data = vec![0f32; frames * NBINS];
    let mut buf = vec![Complex { re: 0f32, im: 0f32 }; FFT_SIZE];
    let mut mag = vec![0f32; FFT_SIZE / 2 + 1];
    for t in 0..frames {
        let chunk = &mono[t * HOP..t * HOP + FFT_SIZE];
        for i in 0..FFT_SIZE {
            buf[i] = Complex { re: chunk[i] * window[i], im: 0.0 };
        }
        fft.process(&mut buf);
        for (m, c) in mag.iter_mut().zip(buf.iter()) {
            *m = c.norm() / FFT_SIZE as f32;
        }
        let out = &mut data[t * NBINS..(t + 1) * NBINS];
        for (b, (lo, hi)) in ranges.iter().enumerate() {
            out[b] = if hi - lo < 1.0 {
                let x = (lo + hi) / 2.0;
                let i = x.floor() as usize;
                let frac = x - i as f32;
                mag[i] * (1.0 - frac) + mag[(i + 1).min(mag.len() - 1)] * frac
            } else {
                let a = lo.ceil() as usize;
                let z = (hi.floor() as usize).min(mag.len() - 1);
                mag[a..=z.max(a)].iter().cloned().fold(0f32, f32::max)
            };
        }
    }
    Spec { frames, data }
}
