use std::f64::consts::PI;

/// Loudness every track is steered toward by `Loudness::gain_db`. Streaming services normalise to -14 LUFS (Spotify,
/// YouTube, Tidal), and club masters of electronic music sit near it, so the suggested gain stays small.
pub const REFERENCE_LUFS: f64 = -14.0;

const ABSOLUTE_GATE_LUFS: f64 = -70.0;
const RELATIVE_GATE_LU: f64 = -10.0;
const HOPS_PER_BLOCK: usize = 4;
const HOPS_PER_SECOND: usize = 10;
const SURROUND_WEIGHT: f64 = 1.41;
const OVERSAMPLE: usize = 4;
const PEAK_TAPS_PER_PHASE: usize = 12;
const PEAK_PROTOTYPE_TAPS: usize = OVERSAMPLE * PEAK_TAPS_PER_PHASE + 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loudness {
    pub lufs: f64,
    pub peak_db: f64,
    pub gain_db: f64,
}

#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Biquad { b, a, z: [0.0; 2] }
    }

    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

fn k_weighting(rate: f64) -> [Biquad; 2] {
    let shelf_f0 = 1681.974450955533;
    let shelf_gain_db = 3.999843853973347;
    let shelf_q = 0.7071752369554196;
    let k = (PI * shelf_f0 / rate).tan();
    let vh = 10f64.powf(shelf_gain_db / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / shelf_q + k * k;
    let shelf = Biquad::new(
        [(vh + vb * k / shelf_q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / shelf_q + k * k) / a0],
        [2.0 * (k * k - 1.0) / a0, (1.0 - k / shelf_q + k * k) / a0],
    );
    let high_pass_f0 = 38.13547087602444;
    let high_pass_q = 0.5003270373238773;
    let k = (PI * high_pass_f0 / rate).tan();
    let a0 = 1.0 + k / high_pass_q + k * k;
    let high_pass = Biquad::new([1.0, -2.0, 1.0], [2.0 * (k * k - 1.0) / a0, (1.0 - k / high_pass_q + k * k) / a0]);
    [shelf, high_pass]
}

const PEAK_CHUNK: usize = 256;
const HISTORY: usize = PEAK_TAPS_PER_PHASE - 1;

struct PeakMeter {
    extended: Vec<f32>,
    peak: f32,
    interpolated: [f32; PEAK_CHUNK],
}

impl PeakMeter {
    fn new() -> Self {
        PeakMeter { extended: vec![0.0; HISTORY], peak: 0.0, interpolated: [0.0; PEAK_CHUNK] }
    }

    fn push(&mut self, block: &[f32], phases: &Phases) {
        self.extended.extend_from_slice(block);
        self.peak = block.iter().fold(self.peak, |m, x| m.max(x.abs()));
        let mut start = 0;
        while start < block.len() {
            let len = PEAK_CHUNK.min(block.len() - start);
            let window = &self.extended[start..start + len + HISTORY];
            let window_peak = window.iter().fold(0f32, |m, x| m.max(x.abs()));
            if window_peak * phases.max_gain > self.peak {
                for taps in &phases.taps[1..] {
                    let acc = &mut self.interpolated[..len];
                    acc.fill(0.0);
                    for (k, &tap) in taps.iter().enumerate() {
                        for (a, &x) in acc.iter_mut().zip(&window[HISTORY - k..]) {
                            *a += tap * x;
                        }
                    }
                    self.peak = acc.iter().fold(self.peak, |m, y| m.max(y.abs()));
                }
            }
            start += len;
        }
        let keep_from = self.extended.len() - HISTORY;
        self.extended.copy_within(keep_from.., 0);
        self.extended.truncate(HISTORY);
    }
}

struct Phases {
    taps: [[f32; PEAK_TAPS_PER_PHASE]; OVERSAMPLE],
    max_gain: f32,
}

fn interpolation_phases() -> Phases {
    let centre = (PEAK_PROTOTYPE_TAPS / 2) as f64;
    let prototype = |n: usize| {
        let x = (n as f64 - centre) / OVERSAMPLE as f64;
        let sinc = if x == 0.0 { 1.0 } else { (PI * x).sin() / (PI * x) };
        let hann = 0.5 * (1.0 - (2.0 * PI * n as f64 / (PEAK_PROTOTYPE_TAPS - 1) as f64).cos());
        sinc * hann
    };
    let mut taps = [[0f32; PEAK_TAPS_PER_PHASE]; OVERSAMPLE];
    for (phase, row) in taps.iter_mut().enumerate() {
        for (k, tap) in row.iter_mut().enumerate() {
            let n = phase + OVERSAMPLE * k;
            *tap = if n < PEAK_PROTOTYPE_TAPS { prototype(n) as f32 } else { 0.0 };
        }
    }
    let max_gain = taps[1..].iter().map(|row| row.iter().map(|t| t.abs()).sum::<f32>()).fold(0.0, f32::max);
    Phases { taps, max_gain }
}

pub fn channel_weight(is_lfe: bool, is_surround: bool) -> f64 {
    if is_lfe {
        0.0
    } else if is_surround {
        SURROUND_WEIGHT
    } else {
        1.0
    }
}

pub fn default_channel_weights(channels: usize) -> Vec<f64> {
    vec![1.0; channels]
}

pub struct Meter {
    weights: Vec<f64>,
    filters: Vec<[Biquad; 2]>,
    peaks: Vec<PeakMeter>,
    phases: Phases,
    channel: Vec<f32>,
    energy: Vec<f64>,
    hop_len: usize,
    hop_fill: usize,
    hop_sum: f64,
    hops: Vec<f64>,
}

impl Meter {
    pub fn new(rate: u32, weights: Vec<f64>) -> Self {
        let channels = weights.len();
        let filters = k_weighting(rate as f64);
        Meter {
            weights,
            filters: vec![filters; channels],
            peaks: (0..channels).map(|_| PeakMeter::new()).collect(),
            phases: interpolation_phases(),
            channel: Vec::new(),
            energy: Vec::new(),
            hop_len: (rate as usize / HOPS_PER_SECOND).max(1),
            hop_fill: 0,
            hop_sum: 0.0,
            hops: Vec::new(),
        }
    }

    pub fn push_interleaved(&mut self, samples: &[f32]) {
        let channels = self.weights.len();
        let frames = samples.len() / channels;
        self.energy.clear();
        self.energy.resize(frames, 0.0);
        for c in 0..channels {
            self.channel.clear();
            self.channel.extend(samples.iter().skip(c).step_by(channels).take(frames));
            self.peaks[c].push(&self.channel, &self.phases);
            let weight = self.weights[c];
            if weight > 0.0 {
                let [mut shelf, mut high_pass] = self.filters[c];
                for (e, &x) in self.energy.iter_mut().zip(&self.channel) {
                    let y = high_pass.tick(shelf.tick(x as f64));
                    *e += weight * y * y;
                }
                self.filters[c] = [shelf, high_pass];
            }
        }
        for &e in &self.energy {
            self.hop_sum += e;
            self.hop_fill += 1;
            if self.hop_fill == self.hop_len {
                self.hops.push(self.hop_sum);
                self.hop_sum = 0.0;
                self.hop_fill = 0;
            }
        }
    }

    pub fn finish(self) -> Option<Loudness> {
        let peak = self.peaks.iter().map(|p| p.peak).fold(0f32, f32::max);
        let block_samples = (self.hop_len * HOPS_PER_BLOCK) as f64;
        let blocks: Vec<f64> = self.hops.windows(HOPS_PER_BLOCK).map(|w| w.iter().sum::<f64>() / block_samples).collect();
        let lufs = integrated_lufs(&blocks)?;
        let peak_db = if peak > 0.0 { 20.0 * (peak as f64).log10() } else { f64::NEG_INFINITY };
        Some(Loudness { lufs, peak_db, gain_db: REFERENCE_LUFS - lufs })
    }
}

fn to_lufs(mean_square: f64) -> f64 {
    -0.691 + 10.0 * mean_square.log10()
}

fn mean_above(blocks: &[f64], gate_lufs: f64) -> Option<f64> {
    let (sum, count) = blocks.iter().filter(|&&z| to_lufs(z) > gate_lufs).fold((0.0, 0usize), |(s, n), &z| (s + z, n + 1));
    (count > 0).then(|| sum / count as f64)
}

fn integrated_lufs(blocks: &[f64]) -> Option<f64> {
    let above_absolute = mean_above(blocks, ABSOLUTE_GATE_LUFS)?;
    let relative_gate = to_lufs(above_absolute) + RELATIVE_GATE_LU;
    mean_above(blocks, relative_gate).map(to_lufs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, hz: f64, dbfs: f64, seconds: f64, phase: f64) -> Vec<f32> {
        let amplitude = 10f64.powf(dbfs / 20.0);
        (0..(rate as f64 * seconds) as usize).map(|i| (amplitude * (2.0 * PI * hz * i as f64 / rate as f64 + phase).sin()) as f32).collect()
    }

    fn interleave(left: &[f32], right: &[f32]) -> Vec<f32> {
        left.iter().zip(right).flat_map(|(&l, &r)| [l, r]).collect()
    }

    fn measure(rate: u32, channels: usize, samples: &[f32]) -> Option<Loudness> {
        let mut meter = Meter::new(rate, default_channel_weights(channels));
        for chunk in samples.chunks(channels * 1000) {
            meter.push_interleaved(chunk);
        }
        meter.finish()
    }

    fn stereo_sine(rate: u32, dbfs: f64, seconds: f64) -> Vec<f32> {
        let s = sine(rate, 997.0, dbfs, seconds, 0.0);
        interleave(&s, &s)
    }

    #[test]
    fn full_scale_997_hz_sine_in_one_channel_is_minus_3_01_lufs() {
        for rate in [44_100, 48_000] {
            let mono = sine(rate, 997.0, 0.0, 10.0, 0.0);
            let l = measure(rate, 1, &mono).unwrap();
            assert!((l.lufs + 3.01).abs() < 0.02, "{rate}: {}", l.lufs);
            assert!(l.peak_db.abs() < 0.05, "{rate}: {}", l.peak_db);
        }
    }

    #[test]
    fn stereo_997_hz_sine_at_minus_23_dbfs_reads_minus_23_lufs() {
        for rate in [44_100, 48_000, 96_000] {
            let l = measure(rate, 2, &stereo_sine(rate, -23.0, 20.0)).unwrap();
            assert!((l.lufs + 23.0).abs() < 0.02, "{rate}: {}", l.lufs);
        }
    }

    #[test]
    fn level_changes_move_lufs_one_for_one() {
        let quiet = measure(48_000, 2, &stereo_sine(48_000, -40.0, 10.0)).unwrap();
        let loud = measure(48_000, 2, &stereo_sine(48_000, -10.0, 10.0)).unwrap();
        assert!((loud.lufs - quiet.lufs - 30.0).abs() < 0.01);
    }

    #[test]
    fn k_weighting_attenuates_low_frequencies_and_lifts_highs() {
        let at = |hz| measure(48_000, 1, &sine(48_000, hz, -20.0, 10.0, 0.0)).unwrap().lufs;
        assert!(at(30.0) < at(997.0) - 3.0);
        assert!(at(10_000.0) > at(997.0) + 3.0);
    }

    #[test]
    fn relative_gate_drops_blocks_more_than_10_lu_below() {
        let mut samples = stereo_sine(48_000, -36.0, 20.0);
        samples.extend(stereo_sine(48_000, -23.0, 20.0));
        let l = measure(48_000, 2, &samples).unwrap();
        assert!((l.lufs + 23.0).abs() < 0.05, "{}", l.lufs);
    }

    #[test]
    fn blocks_within_10_lu_all_count() {
        let mut samples = stereo_sine(48_000, -26.0, 20.0);
        samples.extend(stereo_sine(48_000, -20.0, 20.0));
        let l = measure(48_000, 2, &samples).unwrap();
        let expected = -0.691 + 10.0 * ((10f64.powf((-26.0 + 0.691) / 10.0) + 10f64.powf((-20.0 + 0.691) / 10.0)) / 2.0).log10();
        assert!((l.lufs - expected).abs() < 0.05, "{} vs {expected}", l.lufs);
    }

    #[test]
    fn absolute_gate_drops_blocks_below_minus_70_lufs() {
        let mut samples = stereo_sine(48_000, -80.0, 20.0);
        samples.extend(stereo_sine(48_000, -23.0, 20.0));
        let l = measure(48_000, 2, &samples).unwrap();
        assert!((l.lufs + 23.0).abs() < 0.05, "{}", l.lufs);
    }

    #[test]
    fn silence_and_tracks_shorter_than_one_block_have_no_loudness() {
        assert_eq!(measure(48_000, 2, &vec![0.0; 2 * 48_000 * 5]), None);
        assert_eq!(measure(48_000, 2, &stereo_sine(48_000, -10.0, 0.3)), None);
        assert_eq!(measure(48_000, 2, &stereo_sine(48_000, -90.0, 5.0)), None);
    }

    #[test]
    fn gain_steers_toward_the_reference() {
        let l = measure(48_000, 2, &stereo_sine(48_000, -23.0, 10.0)).unwrap();
        assert!((l.gain_db - 9.0).abs() < 0.02, "{}", l.gain_db);
        let loud = measure(48_000, 2, &stereo_sine(48_000, -8.0, 10.0)).unwrap();
        assert!((loud.gain_db + 6.0).abs() < 0.02, "{}", loud.gain_db);
    }

    #[test]
    fn lfe_channel_is_excluded_and_surrounds_are_lifted() {
        let tone = sine(48_000, 997.0, -20.0, 10.0, 0.0);
        let frames = |per_frame: &dyn Fn(f32) -> Vec<f32>| tone.iter().flat_map(|&x| per_frame(x)).collect::<Vec<f32>>();
        let weights = vec![channel_weight(false, false), channel_weight(true, false)];
        let mut with_lfe = Meter::new(48_000, weights);
        with_lfe.push_interleaved(&frames(&|x| vec![x, x]));
        let mut alone = Meter::new(48_000, vec![1.0]);
        alone.push_interleaved(&tone);
        assert!((with_lfe.finish().unwrap().lufs - alone.finish().unwrap().lufs).abs() < 1e-9);

        let mut surround = Meter::new(48_000, vec![channel_weight(false, true)]);
        surround.push_interleaved(&tone);
        let mut front = Meter::new(48_000, vec![1.0]);
        front.push_interleaved(&tone);
        let lift = surround.finish().unwrap().lufs - front.finish().unwrap().lufs;
        assert!((lift - 1.49).abs() < 0.01, "{lift}");
    }

    #[test]
    fn true_peak_sees_the_overshoot_between_samples() {
        let quarter_rate_sine = sine(48_000, 12_000.0, 0.0, 5.0, std::f64::consts::FRAC_PI_4);
        let sample_peak = quarter_rate_sine.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!((sample_peak as f64 - 0.7071).abs() < 0.001);
        let l = measure(48_000, 1, &quarter_rate_sine).unwrap();
        assert!(l.peak_db > -0.15 && l.peak_db < 0.15, "{}", l.peak_db);
    }

    #[test]
    fn true_peak_is_never_below_the_sample_peak() {
        let square: Vec<f32> = (0..48_000 * 5).map(|i| if (i / 50) % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let l = measure(48_000, 1, &square).unwrap();
        assert!(l.peak_db >= 0.0, "{}", l.peak_db);
    }

    #[test]
    fn k_weighting_filter_matches_the_spec_coefficients_at_48_khz() {
        let [shelf, high_pass] = k_weighting(48_000.0);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-8;
        assert!(close(shelf.b[0], 1.53512485958697) && close(shelf.b[1], -2.69169618940638) && close(shelf.b[2], 1.19839281085285));
        assert!(close(shelf.a[0], -1.69065929318241) && close(shelf.a[1], 0.73248077421585));
        assert!(close(high_pass.a[0], -1.99004745483398) && close(high_pass.a[1], 0.99007225036621));
    }

    #[test]
    fn skipping_quiet_windows_matches_an_exhaustive_true_peak() {
        let mut state = 12345u32;
        let samples: Vec<f32> = (0..48_000 * 3)
            .map(|i| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (state >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
                let swell = 0.05 + 0.9 * ((i as f32 / 20_000.0).sin().abs());
                noise * swell
            })
            .collect();
        let phases = interpolation_phases();
        let mut padded = vec![0f32; HISTORY];
        padded.extend(&samples);
        let exhaustive = (0..samples.len())
            .flat_map(|n| {
                let padded = &padded;
                let phases = &phases;
                (0..OVERSAMPLE).map(move |p| (0..PEAK_TAPS_PER_PHASE).map(|k| phases.taps[p][k] * padded[n + HISTORY - k]).sum::<f32>().abs())
            })
            .fold(0f32, f32::max);
        let mut meter = PeakMeter::new();
        for chunk in samples.chunks(1000) {
            meter.push(chunk, &phases);
        }
        assert!((meter.peak - exhaustive).abs() < 1e-5, "{} vs {exhaustive}", meter.peak);
    }
}
