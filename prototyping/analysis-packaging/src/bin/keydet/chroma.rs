use crate::spectro::{Spec, BINS_PER_SEMITONE, MIDI_LO, NBINS, SEMITONES};

#[derive(Clone, Debug)]
pub struct Params {
    pub hpss: bool,
    pub hpss_time: usize,
    pub hpss_freq: usize,
    pub gamma: f32,
    pub whiten: usize,
    pub harmonics: usize,
    pub harmonic_decay: f32,
    pub segment_frames: usize,
    pub bass_hi: usize,
    pub mid_lo: usize,
    pub mid_hi: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            hpss: false,
            hpss_time: 9,
            hpss_freq: 45,
            gamma: 10.0,
            whiten: 18,
            harmonics: 8,
            harmonic_decay: 0.9,
            segment_frames: 43,
            bass_hi: 52,
            mid_lo: 48,
            mid_hi: 90,
        }
    }
}

pub const CHANNELS: usize = 2;
pub type Chroma = [[f32; 12]; CHANNELS];

pub struct TrackFeatures {
    pub mean: Chroma,
    pub segments: Vec<Chroma>,
    pub tuning_bin: usize,
}

fn median_of(buf: &mut [f32]) -> f32 {
    let mid = buf.len() / 2;
    *buf.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap()).1
}

fn hpss(data: &mut [f32], frames: usize, time_k: usize, freq_k: usize) {
    let mut harmonic = vec![0f32; data.len()];
    let mut percussive = vec![0f32; data.len()];
    let mut buf = Vec::with_capacity(time_k.max(freq_k));
    let (th, fh) = (time_k / 2, freq_k / 2);
    for t in 0..frames {
        for b in 0..NBINS {
            buf.clear();
            for tt in t.saturating_sub(th)..(t + th + 1).min(frames) {
                buf.push(data[tt * NBINS + b]);
            }
            harmonic[t * NBINS + b] = median_of(&mut buf);
            buf.clear();
            for bb in b.saturating_sub(fh)..(b + fh + 1).min(NBINS) {
                buf.push(data[t * NBINS + bb]);
            }
            percussive[t * NBINS + b] = median_of(&mut buf);
        }
    }
    for i in 0..data.len() {
        let (h, p) = (harmonic[i] * harmonic[i], percussive[i] * percussive[i]);
        data[i] *= h / (h + p + 1e-12);
    }
}

fn harmonic_sum(frame: &mut [f32], harmonics: usize, decay: f32) {
    let original = frame.to_vec();
    for h in 2..=harmonics {
        let shift = (36.0 * (h as f32).log2()).round() as usize;
        let weight = decay.powi(h as i32 - 1);
        for b in 0..NBINS.saturating_sub(shift) {
            frame[b] += weight * original[b + shift];
        }
    }
}

fn whiten(frame: &mut [f32], half_width: usize) {
    if half_width == 0 {
        return;
    }
    let original = frame.to_vec();
    let mut prefix = vec![0f32; NBINS + 1];
    for b in 0..NBINS {
        prefix[b + 1] = prefix[b] + original[b];
    }
    for b in 0..NBINS {
        let lo = b.saturating_sub(half_width);
        let hi = (b + half_width + 1).min(NBINS);
        let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f32;
        frame[b] = (original[b] - mean).max(0.0);
    }
}

fn l2_normalise(v: &mut [f32; 12]) -> bool {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n < 1e-9 {
        return false;
    }
    v.iter_mut().for_each(|x| *x /= n);
    true
}

fn fold(frame: &[f32], tuning_bin: usize, lo: usize, hi: usize) -> [f32; 12] {
    let mut out = [0f32; 12];
    for m in lo..hi.min(MIDI_LO + SEMITONES) {
        let centre = BINS_PER_SEMITONE * (m - MIDI_LO) + tuning_bin;
        let mut v = frame[centre];
        if centre > 0 {
            v += 0.5 * frame[centre - 1];
        }
        if centre + 1 < NBINS {
            v += 0.5 * frame[centre + 1];
        }
        out[m % 12] += v;
    }
    out
}

pub fn estimate_tuning(frames: &[Vec<f32>]) -> usize {
    let mut phase = [0f32; BINS_PER_SEMITONE];
    for frame in frames {
        for m in 40..84 {
            for p in 0..BINS_PER_SEMITONE {
                phase[p] += frame[BINS_PER_SEMITONE * (m - MIDI_LO) + p];
            }
        }
    }
    (0..BINS_PER_SEMITONE).max_by(|a, b| phase[*a].partial_cmp(&phase[*b]).unwrap()).unwrap()
}

fn add(acc: &mut Chroma, c: &Chroma) {
    for d in 0..CHANNELS {
        for i in 0..12 {
            acc[d][i] += c[d][i];
        }
    }
}

pub fn features(spec: &Spec, p: &Params) -> TrackFeatures {
    let mut data = spec.data.clone();
    if p.hpss && spec.frames > 2 {
        hpss(&mut data, spec.frames, p.hpss_time, p.hpss_freq);
    }
    let reference = (data.iter().sum::<f32>() / data.len().max(1) as f32).max(1e-9);
    let frames: Vec<Vec<f32>> = (0..spec.frames)
        .map(|t| {
            let mut f: Vec<f32> =
                data[t * NBINS..(t + 1) * NBINS].iter().map(|m| (1.0 + p.gamma * m / reference).ln()).collect();
            if p.harmonics > 1 {
                harmonic_sum(&mut f, p.harmonics, p.harmonic_decay);
            }
            whiten(&mut f, p.whiten);
            f
        })
        .collect();
    let tuning_bin = estimate_tuning(&frames);

    let mut per_frame: Vec<Chroma> = Vec::new();
    for f in &frames {
        let mut c: Chroma = [fold(f, tuning_bin, MIDI_LO + 4, p.bass_hi), fold(f, tuning_bin, p.mid_lo, p.mid_hi)];
        let mut ok = false;
        for ch in c.iter_mut() {
            ok |= l2_normalise(ch);
        }
        if ok {
            per_frame.push(c);
        }
    }

    let mut mean: Chroma = [[0.0; 12]; CHANNELS];
    for c in &per_frame {
        add(&mut mean, c);
    }
    for ch in mean.iter_mut() {
        l2_normalise(ch);
    }

    let mut segments = Vec::new();
    for chunk in per_frame.chunks(p.segment_frames.max(1)) {
        if chunk.len() < p.segment_frames / 2 {
            continue;
        }
        let mut s: Chroma = [[0.0; 12]; CHANNELS];
        for c in chunk {
            add(&mut s, c);
        }
        for ch in s.iter_mut() {
            l2_normalise(ch);
        }
        segments.push(s);
    }
    TrackFeatures { mean, segments, tuning_bin }
}
