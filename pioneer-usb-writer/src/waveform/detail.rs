//! App-only high-resolution 3-band series for the zoomed waveform. Never written to the USB export.
//!
//! The same filters, gains and byte scale as `PWV7`, at [`DETAIL_COLUMNS_PER_SECOND`] columns per second with a
//! release of a few milliseconds instead of the `PWV7` release of about 190 ms, so a kick or a hat keeps its
//! sharp attack and its decay at any zoom. Column `j` covers samples `[round(j * rate / 2400), round((j + 1) * rate / 2400))`;
//! 16 columns make exactly one `PWV7` column, so the per-band maximum over 16 columns equals the raw `PWV7` band peak.

use super::{band_gain, Biquad, Tuning, COLUMNS_PER_SECOND, PWV7_MAX};

pub const DETAIL_COLUMNS_PER_SECOND: f64 = 2400.0;
const COLUMNS_PER_PWV7_COLUMN: usize = (DETAIL_COLUMNS_PER_SECOND / COLUMNS_PER_SECOND) as usize;
const RELEASE_MS: [f64; 3] = [10.0, 5.0, 2.5];
const MAGIC: &[u8; 4] = b"FFWD";
const FORMAT_VERSION: u8 = 1;
const HEADER_LEN: usize = 16;

/// Low, mid and high bytes (0..=127, the `PWV7` scale), one plane per band, `columns_per_second` columns per second.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailSeries {
    pub columns_per_second: u32,
    pub planes: [Vec<u8>; 3],
}

impl DetailSeries {
    pub fn len(&self) -> usize {
        self.planes[0].len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `FFWD`, version, `columns_per_second` (u32 LE), column count (u32 LE), then the low, mid and high planes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.len();
        let mut out = Vec::with_capacity(HEADER_LEN + 3 * n);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&[FORMAT_VERSION, 0, 0, 0]);
        out.extend_from_slice(&self.columns_per_second.to_le_bytes());
        out.extend_from_slice(&(n as u32).to_le_bytes());
        for plane in &self.planes {
            out.extend_from_slice(plane);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC || bytes[4] != FORMAT_VERSION {
            return Err("not a detail waveform file".to_string());
        }
        let columns_per_second = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        let n = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")) as usize;
        if bytes.len() != HEADER_LEN + 3 * n {
            return Err(format!("detail waveform file is {} bytes, expected {}", bytes.len(), HEADER_LEN + 3 * n));
        }
        let plane = |k: usize| bytes[HEADER_LEN + k * n..HEADER_LEN + (k + 1) * n].to_vec();
        Ok(DetailSeries { columns_per_second, planes: [plane(0), plane(1), plane(2)] })
    }
}

/// Measures interleaved audio into raw band peaks per 1/2400 s as it arrives.
pub struct DetailMeter {
    rate: f64,
    channels: usize,
    tuning: Tuning,
    frames: u64,
    next_edge: u64,
    peaks: Vec<[f32; 3]>,
    current: [f64; 3],
    /// Low, mid high-pass, mid low-pass, high.
    filters: [Biquad; 4],
}

impl DetailMeter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self::with_tuning(sample_rate, channels, Tuning::default())
    }

    pub fn with_tuning(sample_rate: u32, channels: usize, tuning: Tuning) -> Self {
        let rate = sample_rate.max(1) as f64;
        DetailMeter {
            rate,
            channels,
            tuning,
            frames: 0,
            next_edge: (rate / DETAIL_COLUMNS_PER_SECOND).round() as u64,
            peaks: Vec::new(),
            current: [0.0; 3],
            filters: [
                Biquad::lowpass(tuning.low_hz, rate),
                Biquad::highpass(tuning.mid_low_hz, rate),
                Biquad::lowpass(tuning.mid_high_hz, rate),
                Biquad::highpass(tuning.high_hz, rate),
            ],
        }
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        let channels = self.channels.max(1);
        let [mut low, mut mid_hp, mut mid_lp, mut high] = self.filters;
        let (mut frames, mut next_edge) = (self.frames, self.next_edge);
        let mut c = self.current;
        for frame in interleaved.chunks_exact(channels) {
            while frames >= next_edge {
                self.peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
                c = [0.0; 3];
                next_edge = ((self.peaks.len() + 1) as f64 * self.rate / DETAIL_COLUMNS_PER_SECOND).round() as u64;
            }
            let x = frame.iter().map(|&s| s as f64).sum::<f64>() / channels as f64;
            c[0] = c[0].max(low.step(x).abs());
            c[1] = c[1].max(mid_lp.step(mid_hp.step(x)).abs());
            c[2] = c[2].max(high.step(x).abs());
            frames += 1;
        }
        self.current = c;
        self.filters = [low, mid_hp, mid_lp, high];
        (self.frames, self.next_edge) = (frames, next_edge);
    }

    /// `ceil(frames * 2400 / rate)` raw band peaks, before any release or gain.
    pub fn raw_peaks(mut self) -> Vec<[f32; 3]> {
        let n = ((self.frames as f64 * DETAIL_COLUMNS_PER_SECOND / self.rate).ceil() as usize).max(1);
        let c = self.current;
        self.peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
        self.peaks.resize(n, [0.0; 3]);
        self.peaks
    }

    pub fn finish(self) -> DetailSeries {
        let tuning = self.tuning;
        shape(&self.raw_peaks(), &tuning)
    }
}

/// The `PWV7` gains (from the `PWV7` envelope rebuilt out of the raw peaks), then the fast envelope and the byte scale.
fn shape(raw: &[[f32; 3]], t: &Tuning) -> DetailSeries {
    let mut gains = [0u16; 3];
    for band in 0..3 {
        let mut env = 0.0f64;
        let mut max = 0.0f64;
        for group in raw.chunks(COLUMNS_PER_PWV7_COLUMN) {
            let peak = group.iter().map(|c| c[band] as f64).fold(0.0, f64::max);
            env = peak.max(env * t.release[band]);
            max = max.max(env);
        }
        gains[band] = band_gain(t, band, max);
    }
    let hop_ms = 1000.0 / DETAIL_COLUMNS_PER_SECOND;
    let mut planes = [Vec::with_capacity(raw.len()), Vec::with_capacity(raw.len()), Vec::with_capacity(raw.len())];
    for band in 0..3 {
        let release = (-hop_ms / RELEASE_MS[band]).exp();
        let mut env = 0.0f64;
        for c in raw {
            env = (c[band] as f64).max(env * release);
            let linear = (t.band_scale[band] * gains[band] as f64 * env).min(PWV7_MAX as f64);
            planes[band].push((PWV7_MAX as f64 * (linear / PWV7_MAX as f64).powf(t.curve[band])).round() as u8);
        }
    }
    DetailSeries { columns_per_second: DETAIL_COLUMNS_PER_SECOND as u32, planes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waveform::{analyze, Meter};

    const RATE: u32 = 44_100;

    fn kick_track(seconds: f64) -> Vec<f32> {
        let frames = (RATE as f64 * seconds) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let t = i as f64 / RATE as f64;
            let beat = (t * 2.0).fract() / 2.0;
            let sweep = 55.0 + 120.0 * (-beat * 40.0).exp();
            let kick = (-beat * 30.0).exp() * (2.0 * std::f64::consts::PI * sweep * beat).sin();
            let hat = if (t * 4.0).fract() < 0.01 { ((i * 7919 % 1000) as f64 / 500.0 - 1.0) * 0.4 } else { 0.0 };
            let x = (0.7 * kick + hat) as f32;
            out.extend_from_slice(&[x, x]);
        }
        out
    }

    fn measure(samples: &[f32]) -> (DetailMeter, Meter) {
        let mut detail = DetailMeter::new(RATE, 2);
        let mut reference = Meter::new(RATE, 2);
        for block in samples.chunks(2 * 4096) {
            detail.push(block);
            reference.push(block);
        }
        (detail, reference)
    }

    #[test]
    fn resolution_is_2400_columns_per_second() {
        let (detail, _) = measure(&kick_track(3.0));
        let series = detail.finish();
        assert_eq!(series.columns_per_second, 2400);
        assert_eq!(series.len(), 7200);
        assert!(series.planes.iter().all(|p| p.len() == 7200));
    }

    #[test]
    fn the_peak_of_sixteen_columns_is_the_raw_pwv7_band_peak() {
        let (detail, reference) = measure(&kick_track(4.0));
        let raw = detail.raw_peaks();
        let columns = reference.finish().band_columns();
        assert_eq!(raw.len(), columns.len() * COLUMNS_PER_PWV7_COLUMN);
        for (i, column) in columns.iter().enumerate() {
            for band in 0..3 {
                let group = &raw[i * COLUMNS_PER_PWV7_COLUMN..(i + 1) * COLUMNS_PER_PWV7_COLUMN];
                let peak = group.iter().map(|c| c[band]).fold(0.0f32, f32::max) as f64;
                assert!((peak - column[band]).abs() < 1e-6, "column {i} band {band}: {peak} vs {}", column[band]);
            }
        }
    }

    #[test]
    fn downsampled_with_the_pwv7_release_it_follows_the_pwv7_low_and_mid_series() {
        let (detail, reference) = measure(&kick_track(8.0));
        let series = detail.finish();
        let pwv7 = analyze(&reference.finish()).pwv7;
        let tuning = Tuning::default();
        for band in 0..2 {
            let mut held = 0.0f64;
            let down: Vec<f64> = series.planes[band]
                .chunks(COLUMNS_PER_PWV7_COLUMN)
                .map(|g| {
                    held = (*g.iter().max().unwrap() as f64).max(held * tuning.release[band]);
                    held
                })
                .collect();
            let expected: Vec<f64> = pwv7.iter().map(|c| c[band] as f64).collect();
            let n = down.len().min(expected.len());
            let (a, b) = (&down[..n], &expected[..n]);
            let mean_a = a.iter().sum::<f64>() / n as f64;
            let mean_b = b.iter().sum::<f64>() / n as f64;
            let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - mean_a) * (y - mean_b)).sum();
            let var_a: f64 = a.iter().map(|x| (x - mean_a).powi(2)).sum();
            let var_b: f64 = b.iter().map(|y| (y - mean_b).powi(2)).sum();
            let r = cov / (var_a.sqrt() * var_b.sqrt());
            assert!(r > 0.97, "band {band}: correlation {r}");
            let max_a = a.iter().cloned().fold(0.0, f64::max);
            let max_b = b.iter().cloned().fold(0.0, f64::max);
            assert!((max_a - max_b).abs() <= 2.0, "band {band}: maxima {max_a} vs {max_b}");
        }
    }

    #[test]
    fn a_kick_attacks_within_a_few_columns_and_decays_faster_than_pwv7() {
        let (detail, reference) = measure(&kick_track(3.0));
        let series = detail.finish();
        let pwv7 = analyze(&reference.finish()).pwv7;
        let onset = 2400;
        let low = &series.planes[0];
        let before = low[onset - 4] as i32;
        let after = low[onset + 24..onset + 40].iter().map(|&v| v as i32).max().unwrap();
        assert!(after - before > 40, "attack: {before} -> {after}");
        let tail_detail = low[onset + 600] as f64;
        let tail_pwv7 = pwv7[(onset + 600) / COLUMNS_PER_PWV7_COLUMN][0] as f64;
        assert!(tail_detail < tail_pwv7, "decay {tail_detail} vs PWV7 {tail_pwv7}");
    }

    #[test]
    fn bytes_round_trip_and_reject_damage() {
        let (detail, _) = measure(&kick_track(1.0));
        let series = detail.finish();
        let bytes = series.to_bytes();
        assert_eq!(bytes.len(), HEADER_LEN + 3 * 2400);
        assert_eq!(DetailSeries::from_bytes(&bytes).unwrap(), series);
        assert!(DetailSeries::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(DetailSeries::from_bytes(b"nope").is_err());
    }

    #[test]
    fn silence_is_all_zero() {
        let mut meter = DetailMeter::new(RATE, 2);
        meter.push(&vec![0.0; 2 * RATE as usize]);
        let series = meter.finish();
        assert_eq!(series.len(), 2400);
        assert!(series.planes.iter().all(|p| p.iter().all(|&v| v == 0)));
    }
}
