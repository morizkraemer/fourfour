use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde_json::{json, Value};

const TARGET_RATE: f64 = 22_050.0;
const FFT: usize = 1024;
const HOP: usize = 512;
const WINDOW_S: f64 = 8.0;
const MIN_S: f64 = 2.0;
const SILENCE_PEAK: f32 = 1e-4;
const PEAK_BLOCK: usize = 2048;
const PEAK_QUANTILE: f64 = 0.999;
const LOW_BAND_HZ: (f64, f64) = (30.0, 150.0);
const HIGH_FROM_HZ: f64 = 2000.0;
const FLUX_TO_HZ: f64 = 5000.0;
const LOG_FLOOR: f32 = 1e-2;
const POWER_FLOOR: f64 = 1e-6;
const FRAME_POWER_FLOOR: f64 = 1e-8;
const ONSET_REACH: usize = 12;
const ONSET_RATIO: f32 = 1.3;
const ONSET_MIN_FLUX: f32 = 0.02;
const TOP_SHARE: f64 = 0.5;

pub const FEATURES: usize = 8;
pub const FEATURE_NAMES: [&str; FEATURES] = ["level_db", "dynamics_db", "flux", "low_db", "high_db", "centroid_log2", "flatness_db", "onset_rate"];

struct Model {
    mean: [f64; FEATURES],
    std: [f64; FEATURES],
    weight: [f64; FEATURES],
    cuts: [f64; 9],
}

const MODEL: Model = Model {
    mean: [-13.08161151515048, 4.547735621358525, 0.4176801688290984, 1.4509072475141411, -13.67445025526177, 10.976866482135696, -17.8730941204655, 5.573080124190665],
    std: [3.724624526062186, 2.759236578318905, 0.06675130721805886, 7.9695915032340965, 5.249777327600913, 0.6920742765320831, 6.358121859495479, 1.5357886343415525],
    weight: [0.8141824697787834, 0.0, 0.0, 0.408900592450646, 0.0, 0.0, 0.9914350016135771, 0.0],
    cuts: [-0.632650563855614, -0.1632504822838907, 0.12262235289933875, 0.32659558204735334, 0.5997853251718521, 0.8072173670401944, 1.068000092772877, 1.261897452611505, 1.5068650751428918],
};

#[derive(Clone, Debug, PartialEq)]
pub struct Energy {
    pub rating: u8,
    pub score: f64,
    pub features: [f64; FEATURES],
    pub window_s: f64,
    pub curve: Vec<f64>,
    pub windows: Vec<[f64; FEATURES]>,
}

fn percentile_peak(samples: &[f32]) -> f32 {
    let mut peaks: Vec<f32> = samples.chunks(PEAK_BLOCK).map(|c| c.iter().fold(0f32, |m, v| m.max(v.abs()))).collect();
    peaks.sort_by(|a, b| a.total_cmp(b));
    peaks[((peaks.len() - 1) as f64 * PEAK_QUANTILE).round() as usize]
}

fn onset_peaks(flux: &[f32], range: &std::ops::Range<usize>) -> usize {
    let mut n = 0;
    for f in range.clone() {
        if f == 0 || f + 1 >= flux.len() || !(flux[f] > flux[f - 1] && flux[f] >= flux[f + 1]) {
            continue;
        }
        let lo = f.saturating_sub(ONSET_REACH);
        let hi = (f + ONSET_REACH + 1).min(flux.len());
        let mean = flux[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        if flux[f] > ONSET_RATIO * mean && flux[f] > ONSET_MIN_FLUX {
            n += 1;
        }
    }
    n
}

fn window_features(samples: &[f32], rate: u32) -> Option<(Vec<[f64; FEATURES]>, f64)> {
    let factor = ((rate as f64 / TARGET_RATE).round() as usize).max(1);
    let ds_rate = rate as f64 / factor as f64;
    let mut ds: Vec<f32> = samples.chunks_exact(factor).map(|c| c.iter().sum::<f32>() / factor as f32).collect();
    if (ds.len() as f64) < MIN_S * ds_rate || ds.len() < FFT + HOP {
        return None;
    }
    let peak = percentile_peak(&ds);
    if peak < SILENCE_PEAK {
        return None;
    }
    let scale = 1.0 / peak;
    ds.iter_mut().for_each(|v| *v *= scale);

    let df = ds_rate / FFT as f64;
    let bin = |hz: f64| (hz / df).ceil() as usize;
    let (low_lo, low_hi) = (bin(LOW_BAND_HZ.0), bin(LOW_BAND_HZ.1));
    let (high_lo, flux_hi, top) = (bin(HIGH_FROM_HZ), bin(FLUX_TO_HZ).min(FFT / 2), FFT / 2);
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT);
    let hann: Vec<f32> = (0..FFT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT as f32).cos()).collect();
    let frames = (ds.len() - FFT) / HOP + 1;
    let mut buf = vec![Complex { re: 0f32, im: 0f32 }; FFT];
    let mut prev = vec![LOG_FLOOR.ln(); top];
    let mut flux = Vec::with_capacity(frames);
    let mut low = Vec::with_capacity(frames);
    let mut rest = Vec::with_capacity(frames);
    let mut high = Vec::with_capacity(frames);
    let mut centroid = Vec::with_capacity(frames);
    let mut flatness = Vec::with_capacity(frames);
    let mut frame_db = Vec::with_capacity(frames);
    for f in 0..frames {
        let chunk = &ds[f * HOP..f * HOP + FFT];
        for (i, c) in buf.iter_mut().enumerate() {
            *c = Complex { re: chunk[i] * hann[i], im: 0.0 };
        }
        fft.process(&mut buf);
        let (mut p_low, mut p_rest, mut p_high, mut num, mut den, mut fl) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f32);
        let (mut sum_p, mut sum_ln) = (0f64, 0f64);
        for k in low_lo..top {
            let mag = buf[k].norm();
            let p = (mag * mag) as f64;
            if k < low_hi {
                p_low += p;
            } else {
                p_rest += p;
            }
            if k >= high_lo {
                p_high += p;
            }
            num += k as f64 * df * mag as f64;
            den += mag as f64;
            let ln = mag.max(LOG_FLOOR).ln();
            if k < flux_hi {
                fl += (ln - prev[k]).max(0.0);
                sum_p += (ln as f64 * 2.0).exp();
                sum_ln += ln as f64 * 2.0;
            }
            prev[k] = ln;
        }
        flux.push(if f == 0 { 0.0 } else { fl / (flux_hi - low_lo) as f32 });
        low.push(p_low);
        rest.push(p_rest);
        high.push(p_high);
        centroid.push(if den > 0.0 { (num / den).max(1.0).log2() } else { 0.0 });
        let bins = (flux_hi - low_lo) as f64;
        flatness.push(10.0 * std::f64::consts::LOG10_E * (sum_ln / bins - (sum_p / bins).ln()));
        let hop_power = ds[f * HOP..(f + 1) * HOP].iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / HOP as f64;
        frame_db.push(10.0 * (hop_power + FRAME_POWER_FLOOR).log10());
    }

    let min_frames = (MIN_S * ds_rate / HOP as f64) as usize;
    let per_window = (WINDOW_S * ds_rate / HOP as f64).round() as usize;
    let n = ((frames as f64 / per_window as f64).round() as usize).max(1);
    let hop_s = HOP as f64 / ds_rate;
    let mut windows = Vec::with_capacity(n);
    for i in 0..n {
        let range = (i * frames / n)..((i + 1) * frames / n);
        if range.len() < min_frames.min(frames) {
            continue;
        }
        let len = range.len() as f64;
        let sum = |v: &[f64]| v[range.clone()].iter().sum::<f64>();
        let (p_low, p_rest, p_high) = (sum(&low), sum(&rest), sum(&high));
        let samples_in = &ds[range.start * HOP..(range.end * HOP + FFT).min(ds.len())];
        let rms = (samples_in.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / samples_in.len() as f64).sqrt();
        let db = &frame_db[range.clone()];
        let db_mean = db.iter().sum::<f64>() / len;
        windows.push([
            20.0 * (rms + 1e-9).log10(),
            (db.iter().map(|v| (v - db_mean).powi(2)).sum::<f64>() / len).sqrt(),
            flux[range.clone()].iter().map(|v| *v as f64).sum::<f64>() / len,
            10.0 * ((p_low + POWER_FLOOR) / (p_rest + POWER_FLOOR)).log10(),
            10.0 * ((p_high + POWER_FLOOR) / (p_low + p_rest + POWER_FLOOR)).log10(),
            centroid[range.clone()].iter().sum::<f64>() / len,
            flatness[range.clone()].iter().sum::<f64>() / len,
            onset_peaks(&flux, &range) as f64 / (len * hop_s),
        ]);
    }
    Some((windows, frames as f64 * hop_s / n as f64))
}

pub fn analyze(samples: &[f32], rate: u32) -> Option<Energy> {
    let (windows, window_s) = window_features(samples, rate)?;
    let scores: Vec<f64> = windows.iter().map(|w| score(&MODEL, w)).collect();
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]));
    let keep = ((scores.len() as f64 * TOP_SHARE).ceil() as usize).max(1);
    let top = &order[..keep];
    let track_score = top.iter().map(|i| scores[*i]).sum::<f64>() / keep as f64;
    let features = std::array::from_fn(|k| top.iter().map(|i| windows[*i][k]).sum::<f64>() / keep as f64);
    Some(Energy { rating: rating(&MODEL, track_score), score: track_score, features, window_s, curve: scores, windows })
}

fn score(model: &Model, row: &[f64; FEATURES]) -> f64 {
    (0..FEATURES).map(|k| model.weight[k] * (row[k] - model.mean[k]) / model.std[k]).sum()
}

fn rating(model: &Model, score: f64) -> u8 {
    1 + model.cuts.iter().filter(|c| score >= **c).count() as u8
}

impl Energy {
    pub fn to_json(&self) -> Value {
        let features: serde_json::Map<String, Value> = FEATURE_NAMES.iter().zip(self.features).map(|(n, v)| (n.to_string(), json!(v))).collect();
        json!({ "rating": self.rating, "score": self.score, "features": features, "window_s": self.window_s, "curve": self.curve })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;
    const LOW: usize = 3;
    const CENTROID: usize = 5;
    const ONSET: usize = 7;

    fn tone(hz: f64, amp: f32, seconds: f64) -> Vec<f32> {
        (0..(RATE as f64 * seconds) as usize).map(|i| amp * (2.0 * std::f64::consts::PI * hz * i as f64 / RATE as f64).sin() as f32).collect()
    }

    fn kicks(bpm: f64, seconds: f64, amp: f32) -> Vec<f32> {
        let period = (RATE as f64 * 60.0 / bpm) as usize;
        let mut out = vec![0f32; (RATE as f64 * seconds) as usize];
        for start in (0..out.len()).step_by(period) {
            for i in 0..(RATE as usize / 8).min(out.len() - start) {
                let t = i as f64 / RATE as f64;
                out[start + i] = amp * ((2.0 * std::f64::consts::PI * 55.0 * t).sin() * (-t * 25.0).exp()) as f32;
            }
        }
        out
    }

    #[test]
    fn silence_and_clips_shorter_than_two_seconds_have_no_energy() {
        assert!(analyze(&vec![0.0; RATE as usize * 10], RATE).is_none());
        assert!(analyze(&tone(220.0, 0.5, 1.0), RATE).is_none());
    }

    #[test]
    fn features_do_not_depend_on_file_gain() {
        let loud = kicks(128.0, 30.0, 0.9);
        let quiet: Vec<f32> = loud.iter().map(|v| v * 0.25).collect();
        let a = analyze(&loud, RATE).unwrap();
        let b = analyze(&quiet, RATE).unwrap();
        for k in 0..FEATURES {
            assert!((a.features[k] - b.features[k]).abs() < 1e-3 * (1.0 + a.features[k].abs()), "{} {} {}", FEATURE_NAMES[k], a.features[k], b.features[k]);
        }
    }

    #[test]
    fn kick_track_has_more_low_end_and_onsets_than_a_pad() {
        let kick = analyze(&kicks(128.0, 30.0, 0.8), RATE).unwrap();
        let pad = analyze(&tone(880.0, 0.5, 30.0), RATE).unwrap();
        assert!(kick.features[LOW] > pad.features[LOW]);
        assert!(kick.features[ONSET] > pad.features[ONSET]);
        assert!(kick.features[CENTROID] < pad.features[CENTROID]);
    }

    #[test]
    fn windows_split_the_track_into_about_eight_seconds() {
        let e = analyze(&kicks(120.0, 64.0, 0.8), RATE).unwrap();
        assert_eq!(e.curve.len(), 8);
        assert_eq!(e.windows.len(), 8);
        assert!((e.window_s - 8.0).abs() < 0.5, "{}", e.window_s);
    }

    #[test]
    fn rating_counts_cut_points_and_is_between_one_and_ten() {
        let model = Model { mean: [0.0; FEATURES], std: [1.0; FEATURES], weight: [0.0; FEATURES], cuts: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0] };
        assert_eq!(rating(&model, -5.0), 1);
        assert_eq!(rating(&model, 1.0), 2);
        assert_eq!(rating(&model, 4.5), 5);
        assert_eq!(rating(&model, 100.0), 10);
    }
}
