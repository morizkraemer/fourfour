use crate::chroma::{Chroma, CHANNELS};

pub const CLASSES: usize = 24;

pub const KRUMHANSL_MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
pub const KRUMHANSL_MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
pub const TEMPERLEY_MAJOR: [f32; 12] = [5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0];
pub const TEMPERLEY_MINOR: [f32; 12] = [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0];

pub fn tonic_of(class: usize) -> usize {
    class % 12
}

pub fn is_minor(class: usize) -> bool {
    class >= 12
}

pub fn camelot(class: usize) -> &'static str {
    const MAJOR: [&str; 12] = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"];
    const MINOR: [&str; 12] = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"];
    if is_minor(class) {
        MINOR[tonic_of(class)]
    } else {
        MAJOR[tonic_of(class)]
    }
}

pub fn class_from_camelot(code: &str) -> Option<usize> {
    (0..CLASSES).find(|c| camelot(*c) == code)
}

fn pearson(a: &[f32; 12], b: &[f32; 12], shift: usize) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut num, mut da, mut db) = (0f32, 0f32, 0f32);
    for i in 0..12 {
        let x = a[(i + shift) % 12] - ma;
        let y = b[i] - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    num / (da.sqrt() * db.sqrt() + 1e-12)
}

pub fn template_scores(chroma: &[f32; 12], major: &[f32; 12], minor: &[f32; 12]) -> [f32; CLASSES] {
    let mut s = [0f32; CLASSES];
    for k in 0..12 {
        s[k] = pearson(chroma, major, k);
        s[12 + k] = pearson(chroma, minor, k);
    }
    s
}

#[derive(Clone)]
pub struct Linear {
    pub weights: [[[f32; 12]; CHANNELS]; 2],
    pub bias: [f32; 2],
}

impl Linear {
    pub fn zero() -> Linear {
        Linear { weights: [[[0.0; 12]; CHANNELS]; 2], bias: [0.0; 2] }
    }

    pub fn logits(&self, c: &Chroma) -> [f32; CLASSES] {
        let mut out = [0f32; CLASSES];
        for mode in 0..2 {
            for k in 0..12 {
                let mut s = self.bias[mode];
                for d in 0..CHANNELS {
                    for p in 0..12 {
                        s += self.weights[mode][d][p] * c[d][(p + k) % 12];
                    }
                }
                out[mode * 12 + k] = s;
            }
        }
        out
    }
}

pub fn log_softmax(l: &[f32; CLASSES]) -> [f32; CLASSES] {
    let m = l.iter().cloned().fold(f32::MIN, f32::max);
    let z = l.iter().map(|x| (x - m).exp()).sum::<f32>().ln() + m;
    let mut out = *l;
    out.iter_mut().for_each(|x| *x -= z);
    out
}

pub fn argmax(l: &[f32; CLASSES]) -> usize {
    (0..CLASSES).max_by(|a, b| l[*a].partial_cmp(&l[*b]).unwrap()).unwrap()
}

pub struct Sample {
    pub chroma: Chroma,
    pub label: usize,
}

const N_PARAMS: usize = 2 * CHANNELS * 12 + 2;

fn flatten(m: &Linear) -> Vec<f32> {
    let mut v = Vec::with_capacity(N_PARAMS);
    for mode in 0..2 {
        for d in 0..CHANNELS {
            v.extend_from_slice(&m.weights[mode][d]);
        }
    }
    v.extend_from_slice(&m.bias);
    v
}

fn unflatten(v: &[f32]) -> Linear {
    let mut m = Linear::zero();
    let mut i = 0;
    for mode in 0..2 {
        for d in 0..CHANNELS {
            m.weights[mode][d].copy_from_slice(&v[i..i + 12]);
            i += 12;
        }
    }
    m.bias.copy_from_slice(&v[i..i + 2]);
    m
}

pub fn fit(samples: &[Sample], l2: f32, epochs: usize, lr: f32) -> Linear {
    let mut theta = vec![0f32; N_PARAMS];
    let (mut m1, mut m2) = (vec![0f32; N_PARAMS], vec![0f32; N_PARAMS]);
    let n = samples.len() as f32;
    for epoch in 1..=epochs {
        let model = unflatten(&theta);
        let mut grad = vec![0f32; N_PARAMS];
        for s in samples {
            let lp = log_softmax(&model.logits(&s.chroma));
            for class in 0..CLASSES {
                let delta = (lp[class].exp() - if class == s.label { 1.0 } else { 0.0 }) / n;
                if delta.abs() < 1e-9 {
                    continue;
                }
                let (mode, k) = (class / 12, class % 12);
                for d in 0..CHANNELS {
                    for p in 0..12 {
                        grad[(mode * CHANNELS + d) * 12 + p] += delta * s.chroma[d][(p + k) % 12];
                    }
                }
                grad[2 * CHANNELS * 12 + mode] += delta;
            }
        }
        for i in 0..2 * CHANNELS * 12 {
            grad[i] += l2 * theta[i];
        }
        let (b1, b2) = (0.9f32, 0.999f32);
        for i in 0..N_PARAMS {
            m1[i] = b1 * m1[i] + (1.0 - b1) * grad[i];
            m2[i] = b2 * m2[i] + (1.0 - b2) * grad[i] * grad[i];
            let mh = m1[i] / (1.0 - b1.powi(epoch as i32));
            let vh = m2[i] / (1.0 - b2.powi(epoch as i32));
            theta[i] -= lr * mh / (vh.sqrt() + 1e-8);
        }
    }
    unflatten(&theta)
}
