use std::time::Instant;

pub const FPS: f64 = 50.0;

#[derive(Clone, Debug)]
pub struct FitParams {
    pub lo: f64,
    pub shift_ms: f64,
    pub comb_refine: bool,
    pub snap_tol: f64,
    pub snap_phase_ms: f64,
    pub snap_wide: f64,
    pub scan_pct: f64,
    pub snap_ratio: f64,
    pub threads: usize,
}

impl Default for FitParams {
    fn default() -> Self {
        FitParams {
            lo: 75.0,
            shift_ms: 0.0,
            comb_refine: true,
            snap_tol: 0.05,
            snap_phase_ms: 15.0,
            snap_wide: 0.5,
            scan_pct: 0.3,
            snap_ratio: 0.97,
            threads: 1,
        }
    }
}

pub struct Fit {
    pub bpm: f64,
    pub beats_s: Vec<f64>,
    pub first_s: f64,
    pub raw_bpm: f64,
    pub raw_beats_s: Vec<f64>,
    pub fit_us: u128,
}

pub struct Window {
    pub offset: f64,
    pub len: f64,
}

pub fn sigmoid(x: f32) -> f64 {
    1.0 / (1.0 + (-(x as f64)).exp())
}

pub fn fold_bpm(mut bpm: f64, lo: f64) -> f64 {
    while bpm < lo {
        bpm *= 2.0;
    }
    while bpm >= 2.0 * lo {
        bpm /= 2.0;
    }
    bpm
}

fn refine_peaks(peaks_s: &[f64], logits: &[f32], offset: f64) -> Vec<f64> {
    peaks_s
        .iter()
        .map(|&t| {
            let f = ((t - offset) * FPS).round() as i64;
            let (mut num, mut den) = (0.0, 0.0);
            for g in f - 2..=f + 2 {
                if g < 0 || g as usize >= logits.len() {
                    continue;
                }
                let p = sigmoid(logits[g as usize]);
                num += p * g as f64;
                den += p;
            }
            if den > 0.0 { num / den / FPS + offset } else { t }
        })
        .collect()
}

fn mode_period(peaks: &[f64]) -> Option<f64> {
    let ibis: Vec<f64> = peaks.windows(2).map(|w| w[1] - w[0]).filter(|d| (0.25..=2.0).contains(d)).collect();
    if ibis.len() < 4 {
        return None;
    }
    let mut best = (0usize, 0.0);
    for &c in &ibis {
        let near: Vec<f64> = ibis.iter().copied().filter(|d| (d - c).abs() < 0.015 * c).collect();
        if near.len() > best.0 {
            best = (near.len(), near.iter().sum::<f64>() / near.len() as f64);
        }
    }
    Some(best.1)
}

fn fit_line(peaks: &[f64], p0: f64) -> (f64, f64) {
    let mut k = vec![0f64; peaks.len()];
    for i in 1..peaks.len() {
        k[i] = k[i - 1] + ((peaks[i] - peaks[i - 1]) / p0).round();
    }
    let (mut t0, mut p) = (peaks[0], p0);
    for tol in [0.05, 0.03, 0.02, 0.015].iter() {
        let inl: Vec<usize> = (0..peaks.len()).filter(|&i| (peaks[i] - (t0 + k[i] * p)).abs() < *tol).collect();
        if inl.len() < 4 {
            break;
        }
        let n = inl.len() as f64;
        let mk = inl.iter().map(|&i| k[i]).sum::<f64>() / n;
        let mt = inl.iter().map(|&i| peaks[i]).sum::<f64>() / n;
        let sxx: f64 = inl.iter().map(|&i| (k[i] - mk).powi(2)).sum();
        let sxy: f64 = inl.iter().map(|&i| (k[i] - mk) * (peaks[i] - mt)).sum();
        if sxx == 0.0 {
            break;
        }
        p = sxy / sxx;
        t0 = mt - p * mk;
    }
    (t0, p)
}

fn comb_score(act: &[f64], w: &Window, t0: f64, p: f64) -> f64 {
    let mut s = 0.0;
    let mut t = t0 + ((w.offset - t0) / p).ceil() * p;
    while t < w.offset + w.len {
        let x = (t - w.offset) * FPS;
        let i = x.floor() as usize;
        if i + 1 < act.len() {
            let fr = x - i as f64;
            s += act[i] * (1.0 - fr) + act[i + 1] * fr;
        }
        t += p;
    }
    s
}

fn refine_comb(act: &[f64], w: &Window, t0: f64, p: f64) -> (f64, f64) {
    let mid = w.offset + w.len / 2.0;
    let (mut bt0, mut bp, mut best) = (t0, p, comb_score(act, w, t0, p));
    let mut step_p = p * 2e-4;
    let mut step_t = 0.004;
    for _ in 0..4 {
        let mut improved = true;
        while improved {
            improved = false;
            for dp in [-step_p, 0.0, step_p] {
                for dt in [-step_t, 0.0, step_t] {
                    if dp == 0.0 && dt == 0.0 {
                        continue;
                    }
                    let cp = bp + dp;
                    let ct = bt0 + dt - dp * ((mid - bt0) / bp);
                    let s = comb_score(act, w, ct, cp);
                    if s > best {
                        best = s;
                        bp = cp;
                        bt0 = ct;
                        improved = true;
                    }
                }
            }
        }
        step_p /= 3.0;
        step_t /= 3.0;
    }
    (bt0, bp)
}

fn scan_tempo(act: &[f64], w: &Window, t0: f64, p: f64, steps: i64, threads: usize) -> (f64, f64) {
    let mid = w.offset + w.len / 2.0;
    let k = (mid - t0) / p;
    let total = 2 * steps + 1;
    let threads = threads.max(1).min(total as usize);
    let per = (total as usize).div_ceil(threads) as i64;
    let scan = |from: i64, to: i64| {
        let mut best = (f64::MIN, t0, p);
        for i in from..to {
            let pi = p * (1.0 + (i - steps) as f64 * 1e-5);
            let tc = t0 + (p - pi) * k;
            let mut dt = -pi / 2.0;
            while dt < pi / 2.0 {
                let s = comb_score(act, w, tc + dt, pi);
                if s > best.0 {
                    best = (s, tc + dt, pi);
                }
                dt += 0.004;
            }
        }
        best
    };
    let parts: Vec<(f64, f64, f64)> = if threads == 1 {
        vec![scan(0, total)]
    } else {
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads as i64)
                .map(|j| {
                    let scan = &scan;
                    s.spawn(move || scan(j * per, ((j + 1) * per).min(total)))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        })
    };
    let mut best = (f64::MIN, t0, p);
    for part in parts {
        if part.0 > best.0 {
            best = part;
        }
    }
    (best.1, best.2)
}

pub fn fit_track(
    peaks_s: &[f64],
    beat_logits: &[f32],
    down_logits: &[f32],
    duration: f64,
    w: &Window,
    params: &FitParams,
) -> Option<Fit> {
    let started = Instant::now();
    let refined = refine_peaks(peaks_s, beat_logits, w.offset);
    let p0 = mode_period(&refined)?;
    let raw_bpm = fold_bpm(60.0 / p0, params.lo);
    let (mut t0, mut p) = fit_line(&refined, p0);
    let act: Vec<f64> = beat_logits.iter().map(|&x| sigmoid(x)).collect();
    if params.comb_refine {
        if params.scan_pct > 0.0 {
            let steps = (params.scan_pct / 100.0 / 1e-5) as i64;
            let r = scan_tempo(&act, w, t0, p, steps, params.threads);
            t0 = r.0;
            p = r.1;
        }
        let r = refine_comb(&act, w, t0, p);
        t0 = r.0;
        p = r.1;
    }

    if params.snap_tol > 0.0 {
        let folded = fold_bpm(60.0 / p, params.lo);
        let snapped = folded.round();
        let delta = (folded - snapped).abs();
        if delta < params.snap_wide.max(params.snap_tol) {
            let mid = w.offset + w.len / 2.0;
            let p_new = p * folded / snapped;
            let t_new = t0 + (p - p_new) * ((mid - t0) / p);
            let mut best = (comb_score(&act, w, t_new, p_new), t_new);
            let mut dt = -params.snap_phase_ms / 1000.0;
            while dt <= params.snap_phase_ms / 1000.0 {
                let s = comb_score(&act, w, t_new + dt, p_new);
                if s > best.0 {
                    best = (s, t_new + dt);
                }
                dt += 0.0005;
            }
            let current = comb_score(&act, w, t0, p);
            if delta < params.snap_tol || best.0 >= params.snap_ratio * current {
                p = p_new;
                t0 = best.1;
            }
        }
    }

    let model_bpm = 60.0 / p;
    let target = fold_bpm(model_bpm, params.lo);
    let ratio = target / model_bpm;
    let mut p_out = p / ratio;
    let mut t0_out = t0;
    if ratio < 1.0 {
        let n = (1.0 / ratio).round() as usize;
        let down: Vec<f64> = down_logits.iter().map(|&x| sigmoid(x)).collect();
        let mut best = (f64::MIN, t0);
        for j in 0..n {
            let tj = t0 + j as f64 * p;
            let s = comb_score(&down, w, tj, p * n as f64);
            if s > best.0 {
                best = (s, tj);
            }
        }
        t0_out = best.1;
        p_out = p * n as f64;
    }

    let shift = params.shift_ms / 1000.0;
    let mut first = (t0_out + shift) % p_out;
    if first < 0.0 {
        first += p_out;
    }
    let mut beats = Vec::new();
    let mut t = if first - p_out >= -0.05 { first - p_out } else { first };
    while t < duration + 0.03 {
        beats.push(t);
        t += p_out;
    }
    Some(Fit {
        bpm: 60.0 / p_out,
        beats_s: beats,
        first_s: first,
        raw_bpm,
        raw_beats_s: peaks_s.to_vec(),
        fit_us: started.elapsed().as_micros(),
    })
}

pub fn finalize_beats(beats_s: &[f64], delay_s: f64) -> Vec<f64> {
    beats_s.iter().map(|t| t + delay_s).filter(|t| *t > -0.04).map(|t| t.max(0.0)).collect()
}
