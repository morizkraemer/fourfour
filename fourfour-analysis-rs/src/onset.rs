use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const WIN: usize = 512;
const HOP_S: f64 = 0.01;
const TARGET_RATE: f64 = 11025.0;
pub const BAND_EDGES: [(usize, usize); 3] = [(1, 10), (10, 64), (64, 256)];

pub struct Envelopes {
    pub hop_s: f64,
    pub centre_s: f64,
    pub bands: [Vec<f32>; 3],
}

pub fn envelopes(samples: &[f32], rate: u32) -> Envelopes {
    let factor = ((rate as f64 / TARGET_RATE).round() as usize).max(1);
    let ds_rate = rate as f64 / factor as f64;
    let ds: Vec<f32> = samples.chunks_exact(factor).map(|c| c.iter().sum::<f32>() / factor as f32).collect();
    let hop = (ds_rate * HOP_S).round() as usize;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(WIN);
    let window: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WIN as f32).cos()).collect();
    let frames = ds.len().saturating_sub(WIN) / hop;
    let mut prev = vec![0f32; WIN / 2];
    let mut buf = vec![Complex { re: 0f32, im: 0f32 }; WIN];
    let mut bands = [Vec::with_capacity(frames), Vec::with_capacity(frames), Vec::with_capacity(frames)];
    for f in 0..frames {
        let chunk = &ds[f * hop..f * hop + WIN];
        for (i, c) in buf.iter_mut().enumerate() {
            *c = Complex { re: chunk[i] * window[i], im: 0.0 };
        }
        fft.process(&mut buf);
        let mut flux = [0f32; 3];
        for (b, (lo, hi)) in BAND_EDGES.iter().enumerate() {
            for k in *lo..*hi {
                let m = (1.0 + 100.0 * buf[k].norm()).ln();
                flux[b] += (m - prev[k]).max(0.0);
            }
        }
        for k in 1..WIN / 2 {
            prev[k] = (1.0 + 100.0 * buf[k].norm()).ln();
        }
        for b in 0..3 {
            bands[b].push(flux[b]);
        }
    }
    Envelopes { hop_s: hop as f64 / ds_rate, centre_s: WIN as f64 / 2.0 / ds_rate, bands }
}

const SEARCH_S: f64 = 0.03;

impl Envelopes {
    pub fn phase_energy(&self, band: usize, beats_s: &[f64], shift_s: f64) -> f64 {
        let env = &self.bands[band];
        let reach = (SEARCH_S / self.hop_s).round() as i64;
        let (mut sum, mut n) = (0.0, 0usize);
        for &t in beats_s {
            let x = ((t + shift_s - self.centre_s) / self.hop_s).round() as i64;
            if x - reach < 0 || (x + reach) as usize >= env.len() {
                continue;
            }
            sum += (x - reach..=x + reach).map(|i| env[i as usize]).fold(0f32, f32::max) as f64;
            n += 1;
        }
        if n == 0 { 0.0 } else { sum / n as f64 }
    }

    pub fn offbeat_ratio(&self, band: usize, beats_s: &[f64], period_s: f64) -> f64 {
        let on = self.phase_energy(band, beats_s, 0.0);
        let off = self.phase_energy(band, beats_s, period_s / 2.0);
        off / on.max(1e-9)
    }
}
