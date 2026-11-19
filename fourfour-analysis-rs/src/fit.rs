use std::time::Instant;

pub const FPS: f64 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempoSearch {
    NearLine,
    LineAndGlobalPeak,
}

pub const TEMPO_SEARCH: TempoSearch = TempoSearch::LineAndGlobalPeak;
pub const GLOBAL_SEARCH_PCT: f64 = 4.0;
pub const HALF_TIME_BAND: (f64, f64) = (0.0, 0.0);

#[derive(Clone, Debug)]
pub struct FitParams {
    pub lo: f64,
    pub shift_ms: f64,
    pub comb_refine: bool,
    pub snap_tol: f64,
    pub snap_tol_unchecked: bool,
    pub snap_phase_ms: f64,
    pub tempo_search: TempoSearch,
    pub global_pct: f64,
    pub snap_wide: f64,
    pub scan_pct: f64,
    pub far_skip_pct: f64,
    pub snap_ratio: f64,
    pub keep_native: (f64, f64),
    pub half_time_band: (f64, f64),
    pub threads: usize,
}

impl Default for FitParams {
    fn default() -> Self {
        FitParams {
            lo: 75.0,
            shift_ms: 0.0,
            comb_refine: true,
            snap_tol: 0.05,
            snap_tol_unchecked: false,
            snap_phase_ms: 15.0,
            tempo_search: TEMPO_SEARCH,
            global_pct: GLOBAL_SEARCH_PCT,
            snap_wide: 1.5,
            scan_pct: 0.3,
            far_skip_pct: 0.05,
            snap_ratio: 0.97,
            keep_native: (150.0, 185.0),
            half_time_band: HALF_TIME_BAND,
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

const FOLD_STEP_REL: f64 = 1e-4;
const FOLD_BIN_S: f64 = 0.004;
const FOLD_WINDOW_BINS: usize = 5;

fn fold_score(act: &[f64], p: f64, hist: &mut Vec<f64>) -> f64 {
    let bins = ((p / FOLD_BIN_S).round() as usize).max(FOLD_WINDOW_BINS);
    hist.clear();
    hist.resize(bins, 0.0);
    let per_bin = bins as f64 / p;
    let frame = 1.0 / FPS;
    let mut phase = 0.0;
    for &a in act {
        hist[((phase * per_bin) as usize).min(bins - 1)] += a;
        phase += frame;
        if phase >= p {
            phase -= p;
        }
    }
    let mut run: f64 = hist[bins - FOLD_WINDOW_BINS..].iter().sum();
    let mut best = run;
    for i in 0..bins {
        run += hist[i] - hist[(i + bins - FOLD_WINDOW_BINS) % bins];
        best = best.max(run);
    }
    best
}

fn global_fold_peak(act: &[f64], w: &Window, p: f64, pct: f64) -> f64 {
    let span = &act[..act.len().min((w.len * FPS) as usize)];
    let steps = (pct / 100.0 / FOLD_STEP_REL) as i64;
    let mut hist = Vec::new();
    let mut best = (f64::MIN, p);
    for i in -steps..=steps {
        let pi = p * (1.0 + i as f64 * FOLD_STEP_REL);
        let s = fold_score(span, pi, &mut hist);
        if s > best.0 {
            best = (s, pi);
        }
    }
    best.1
}

const COARSE_STRIDE: i64 = 6;
const COARSE_KEEP: usize = 5;
const FINE_PHASE_STEPS: i64 = 3;
const PHASE_STEP: f64 = 0.004;

fn scan_tempo(act: &[f64], w: &Window, t0: f64, p: f64, steps: i64, threads: usize) -> (f64, f64) {
    let mid = w.offset + w.len / 2.0;
    let k = (mid - t0) / p;
    let total = 2 * steps + 1;
    let period_at = |i: i64| p * (1.0 + (i - steps) as f64 * 1e-5);
    let centre_at = |i: i64| t0 + (p - period_at(i)) * k;
    let phases_at = |i: i64| ((period_at(i) / PHASE_STEP).ceil() as i64).max(1);
    let eval = |i: i64, j: i64| {
        let pi = period_at(i);
        let t = centre_at(i) - pi / 2.0 + j as f64 * PHASE_STEP;
        (comb_score(act, w, t, pi), t, pi)
    };
    let best_phase = |i: i64, from: i64, to: i64| {
        let mut best = (f64::MIN, t0, p, 0i64);
        for j in from..to {
            let (s, t, pi) = eval(i, j);
            if s > best.0 {
                best = (s, t, pi, j);
            }
        }
        best
    };
    let coarse_idx: Vec<i64> = (0..total).step_by(COARSE_STRIDE as usize).collect();
    let run = |idx: &[i64], f: &(dyn Fn(i64) -> (f64, f64, f64, i64) + Sync)| -> Vec<(f64, f64, f64, i64, i64)> {
        let threads = threads.max(1).min(idx.len().max(1));
        if threads == 1 {
            return idx.iter().map(|&i| { let r = f(i); (r.0, r.1, r.2, r.3, i) }).collect();
        }
        let per = idx.len().div_ceil(threads);
        std::thread::scope(|s| {
            let handles: Vec<_> = idx
                .chunks(per)
                .map(|c| s.spawn(move || c.iter().map(|&i| { let r = f(i); (r.0, r.1, r.2, r.3, i) }).collect::<Vec<_>>()))
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        })
    };
    let mut coarse = run(&coarse_idx, &|i| best_phase(i, 0, phases_at(i)));
    coarse.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.4.cmp(&b.4)));
    let mut fine_idx: Vec<i64> = Vec::new();
    let mut fine_phase: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for c in coarse.iter().take(COARSE_KEEP) {
        for i in (c.4 - COARSE_STRIDE + 1).max(0)..(c.4 + COARSE_STRIDE).min(total) {
            if fine_phase.insert(i, c.3).is_none() {
                fine_idx.push(i);
            }
        }
    }
    fine_idx.sort();
    let fine = run(&fine_idx, &|i| {
        let j = fine_phase[&i];
        best_phase(i, (j - FINE_PHASE_STEPS).max(0), (j + FINE_PHASE_STEPS + 1).min(phases_at(i)))
    });
    let mut best = (f64::MIN, t0, p, 0, i64::MAX);
    for r in coarse.into_iter().chain(fine) {
        if r.0 > best.0 || (r.0 == best.0 && r.4 < best.4) {
            best = r;
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
        let scan_from = |t0: f64, p: f64| {
            let (t0, p) = if params.scan_pct > 0.0 {
                let steps = (params.scan_pct / 100.0 / 1e-5) as i64;
                scan_tempo(&act, w, t0, p, steps, params.threads)
            } else {
                (t0, p)
            };
            refine_comb(&act, w, t0, p)
        };
        let near = scan_from(t0, p);
        (t0, p) = near;
        if params.tempo_search == TempoSearch::LineAndGlobalPeak {
            let peak = global_fold_peak(&act, w, p, params.global_pct);
            if (peak / p - 1.0).abs() > params.far_skip_pct / 100.0 {
                let mid = w.offset + w.len / 2.0;
                let far = scan_from(t0 + (p - peak) * ((mid - t0) / p), peak);
                if comb_score(&act, w, far.0, far.1) > comb_score(&act, w, near.0, near.1) {
                    (t0, p) = far;
                }
            }
        }
    }

    if params.snap_tol > 0.0 {
        let folded = fold_bpm(60.0 / p, params.lo);
        let mid = w.offset + w.len / 2.0;
        let current = comb_score(&act, w, t0, p);
        let reach = params.snap_wide.max(params.snap_tol);
        let mut candidates: Vec<f64> = ((folded - reach).ceil() as i64..=(folded + reach).floor() as i64).map(|n| n as f64).collect();
        candidates.sort_by(|a, b| (a - folded).abs().total_cmp(&(b - folded).abs()));
        for snapped in candidates {
            let delta = (folded - snapped).abs();
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
            let unchecked = params.snap_tol_unchecked && delta < params.snap_tol;
            if unchecked || best.0 >= params.snap_ratio * current {
                p = p_new;
                t0 = best.1;
                break;
            }
        }
    }

    let model_bpm = 60.0 / p;
    let keep = model_bpm >= params.keep_native.0 && model_bpm <= params.keep_native.1;
    let target = if keep { model_bpm } else { fold_bpm(model_bpm, params.lo) };
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

    let out_bpm = 60.0 / p_out;
    if out_bpm >= params.half_time_band.0 && out_bpm <= params.half_time_band.1 {
        p_out /= 2.0;
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

#[derive(Clone, Debug)]
pub struct WindowCheck {
    pub from_s: f64,
    pub to_s: f64,
    pub expected: f64,
    pub peaks: usize,
    pub inliers: usize,
    pub median_ms: f64,
}

pub const WINDOW_INLIER_S: f64 = 0.035;
pub const WINDOW_BEATLESS_SHARE: f64 = 0.25;
pub const WINDOW_MIN_SHARE: f64 = 0.6;
pub const WINDOW_MAX_MEDIAN_MS: f64 = 15.0;
pub const WINDOW_MAX_SPREAD_MS: f64 = 10.0;

pub fn check_windows(fit: &Fit, peaks_s: &[f64], beat_logits: &[f32], spans: &[(usize, usize)]) -> Vec<WindowCheck> {
    let period = 60.0 / fit.bpm;
    let refined = refine_peaks(peaks_s, beat_logits, 0.0);
    spans
        .iter()
        .map(|&(from, to)| {
            let (from_s, to_s) = (from as f64 / FPS, to as f64 / FPS);
            let inside: Vec<f64> = refined.iter().copied().filter(|&t| t >= from_s && t < to_s).collect();
            let mut offsets: Vec<f64> = inside
                .iter()
                .map(|&t| t - (fit.first_s + ((t - fit.first_s) / period).round() * period))
                .filter(|d| d.abs() < WINDOW_INLIER_S)
                .collect();
            offsets.sort_by(f64::total_cmp);
            let median_ms = offsets.get(offsets.len() / 2).map_or(f64::INFINITY, |d| d * 1000.0);
            WindowCheck { from_s, to_s, expected: (to_s - from_s) / period, peaks: inside.len(), inliers: offsets.len(), median_ms }
        })
        .collect()
}

pub fn windows_agree(checks: &[WindowCheck], tolerated_weak: usize, max_spread_ms: f64) -> bool {
    let with_beats = checks.iter().filter(|c| c.peaks as f64 >= WINDOW_BEATLESS_SHARE * c.expected);
    let (good, weak): (Vec<&WindowCheck>, Vec<&WindowCheck>) = with_beats.partition(|c| c.inliers as f64 >= WINDOW_MIN_SHARE * c.expected);
    let medians = good.iter().map(|c| c.median_ms);
    let spread = medians.clone().fold(f64::MIN, f64::max) - medians.fold(f64::MAX, f64::min);
    good.len() >= 2 + tolerated_weak
        && weak.len() <= tolerated_weak
        && spread <= max_spread_ms
        && good.iter().all(|c| c.median_ms.abs() <= WINDOW_MAX_MEDIAN_MS)
}

pub fn finalize_beats(beats_s: &[f64], delay_s: f64) -> Vec<f64> {
    beats_s.iter().map(|t| t + delay_s).filter(|t| *t > -0.04).map(|t| t.max(0.0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DURATION: f64 = 420.0;

    fn synthetic(bpm: f64) -> (Vec<f64>, Vec<f32>) {
        let period = 60.0 / bpm;
        let first = 0.137;
        let frames = (DURATION * FPS) as usize;
        let beats: Vec<f64> = (0..).map(|k| first + k as f64 * period).take_while(|&t| t < DURATION).collect();
        let logits: Vec<f32> = (0..frames)
            .map(|f| {
                let t = f as f64 / FPS;
                let k = ((t - first) / period).round();
                let dt = t - (first + k * period);
                let p = 0.001 + 0.998 * (-(dt / 0.012).powi(2) / 2.0).exp();
                (p / (1.0 - p)).ln() as f32
            })
            .collect();
        let peaks = beats.iter().map(|t| (t * FPS).round() / FPS).collect();
        (peaks, logits)
    }

    fn fitted_bpm(bpm: f64, params: &FitParams) -> f64 {
        let (peaks, logits) = synthetic(bpm);
        let w = Window { offset: 0.0, len: logits.len() as f64 / FPS };
        fit_track(&peaks, &logits, &logits, DURATION, &w, params).unwrap().bpm
    }

    #[test]
    fn integer_tempo_snaps_to_the_integer() {
        assert_eq!(fitted_bpm(128.0, &FitParams::default()), 128.0);
    }

    #[test]
    fn tempo_just_off_an_integer_is_not_snapped_when_the_comb_disagrees() {
        let bpm = fitted_bpm(128.02, &FitParams::default());
        assert!((bpm - 128.02).abs() < 0.003, "fitted {bpm}");
    }

    fn fit_from_peaks_off_the_true_tempo(tempo_search: TempoSearch) -> f64 {
        let (_, logits) = synthetic(114.0);
        let (peaks, _) = synthetic(116.4);
        let w = Window { offset: 0.0, len: logits.len() as f64 / FPS };
        let params = FitParams { tempo_search, ..FitParams::default() };
        fit_track(&peaks, &logits, &logits, DURATION, &w, &params).unwrap().bpm
    }

    #[test]
    fn global_peak_finds_the_tempo_outside_the_near_scan() {
        let near = fit_from_peaks_off_the_true_tempo(TempoSearch::NearLine);
        let global = fit_from_peaks_off_the_true_tempo(TempoSearch::LineAndGlobalPeak);
        assert!((near - 114.0).abs() > 0.1, "near-only fitted {near}");
        assert_eq!(global, 114.0);
    }

    fn fit_from_peaks_just_outside_the_near_scan(far_skip_pct: f64) -> f64 {
        let (_, strong) = synthetic(128.0);
        let (_, weak) = synthetic(128.3);
        let logits: Vec<f32> = strong.iter().zip(&weak).map(|(&s, &w)| s.max(w - 1.0)).collect();
        let (peaks, _) = synthetic(128.6);
        let w = Window { offset: 0.0, len: logits.len() as f64 / FPS };
        let params = FitParams { far_skip_pct, snap_tol: 0.0, ..FitParams::default() };
        fit_track(&peaks, &logits, &logits, DURATION, &w, &params).unwrap().bpm
    }

    #[test]
    fn global_peak_is_searched_even_when_it_is_close_to_the_near_result() {
        let skipped = fit_from_peaks_just_outside_the_near_scan(0.3);
        let searched = fit_from_peaks_just_outside_the_near_scan(FitParams::default().far_skip_pct);
        assert!((skipped - 128.0).abs() > 0.05, "skipped fitted {skipped}");
        assert!((searched - 128.0).abs() < 0.003, "searched fitted {searched}");
    }

    #[test]
    fn half_time_tempo_in_the_band_is_doubled() {
        let on = FitParams { half_time_band: (84.0, 92.5), ..FitParams::default() };
        assert_eq!(fitted_bpm(88.0, &on), 176.0);
        assert_eq!(fitted_bpm(88.0, &FitParams::default()), 88.0);
        assert_eq!(fitted_bpm(80.0, &on), 80.0);
        assert_eq!(fitted_bpm(94.0, &on), 94.0);
    }

    const SPANS: [(usize, usize); 3] = [(4000, 5500), (10000, 11500), (16000, 17500)];

    fn windowed(shift_middle_frames: usize) -> (Fit, Vec<f64>, Vec<f32>) {
        let (_, full) = synthetic(128.0);
        let mut logits = vec![-1000.0f32; full.len()];
        for (n, &(from, to)) in SPANS.iter().enumerate() {
            let shift = if n == 1 { shift_middle_frames } else { 0 };
            logits[from..to].copy_from_slice(&full[from - shift..to - shift]);
        }
        let peaks: Vec<f64> = (1..logits.len() - 1)
            .filter(|&f| logits[f] > 0.0 && logits[f] >= logits[f - 1] && logits[f] > logits[f + 1])
            .map(|f| f as f64 / FPS)
            .collect();
        let w = Window { offset: 0.0, len: logits.len() as f64 / FPS };
        let fit = fit_track(&peaks, &logits, &logits, DURATION, &w, &FitParams::default()).unwrap();
        (fit, peaks, logits)
    }

    #[test]
    fn windows_of_a_steady_track_agree_on_its_tempo() {
        let (fit, peaks, logits) = windowed(0);
        assert!((fit.bpm - 128.0).abs() < 1e-9, "fitted {}", fit.bpm);
        assert!(windows_agree(&check_windows(&fit, &peaks, &logits, &SPANS), 0, WINDOW_MAX_SPREAD_MS));
    }

    #[test]
    fn a_window_off_the_grid_of_the_others_disagrees() {
        let (fit, peaks, logits) = windowed(1);
        assert!(!windows_agree(&check_windows(&fit, &peaks, &logits, &SPANS), 0, WINDOW_MAX_SPREAD_MS));
    }

    #[test]
    fn unchecked_snap_keeps_the_old_rule() {
        let params = FitParams { snap_tol_unchecked: true, ..FitParams::default() };
        assert_eq!(fitted_bpm(128.02, &params), 128.0);
    }
}
