use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{json, Value};

const FPS: f64 = 50.0;

struct Params {
    lo: f64,
    shift_ms: f64,
    comb_refine: bool,
}

struct Reference {
    id: u64,
    artist: String,
    title: String,
    bpm: f64,
    beats_ms: Vec<f64>,
    constant: bool,
}

struct Fit {
    bpm: f64,
    beats_s: Vec<f64>,
    first_s: f64,
    raw_bpm: f64,
    raw_beats_s: Vec<f64>,
    fit_us: u128,
}

fn sigmoid(x: f32) -> f64 {
    1.0 / (1.0 + (-(x as f64)).exp())
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
    for (iter, tol) in [0.05, 0.03, 0.02, 0.015].iter().enumerate() {
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
        let _ = iter;
    }
    (t0, p)
}

struct Window {
    offset: f64,
    len: f64,
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


fn fit_track(
    peaks_s: &[f64],
    beat_logits: &[f32],
    down_logits: &[f32],
    duration: f64,
    w: &Window,
    params: &Params,
) -> Option<Fit> {
    let started = Instant::now();
    let refined = refine_peaks(peaks_s, beat_logits, w.offset);
    let p0 = mode_period(&refined)?;
    let raw_bpm = fold_bpm(60.0 / p0, params.lo);
    let (mut t0, mut p) = fit_line(&refined, p0);
    if params.comb_refine {
        let act: Vec<f64> = beat_logits.iter().map(|&x| sigmoid(x)).collect();
        let r = refine_comb(&act, w, t0, p);
        t0 = r.0;
        p = r.1;
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
    let mut t = first;
    while t < duration {
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

fn fold_bpm(mut bpm: f64, lo: f64) -> f64 {
    while bpm < lo {
        bpm *= 2.0;
    }
    while bpm >= 2.0 * lo {
        bpm /= 2.0;
    }
    bpm
}

fn match_f(reference: &[f64], est: &[f64], tol_ms: f64) -> (usize, f64) {
    let (mut i, mut j, mut tp) = (0, 0, 0);
    while i < reference.len() && j < est.len() {
        let d = est[j] - reference[i];
        if d.abs() <= tol_ms {
            tp += 1;
            i += 1;
            j += 1;
        } else if d < 0.0 {
            j += 1;
        } else {
            i += 1;
        }
    }
    let denom = (reference.len() + est.len()) as f64;
    (tp, if denom > 0.0 { 2.0 * tp as f64 / denom } else { 0.0 })
}

fn nearest_offsets(reference: &[f64], est: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(reference.len());
    let mut j = 0;
    for &r in reference {
        while j + 1 < est.len() && est[j + 1] <= r {
            j += 1;
        }
        let mut best = f64::MAX;
        for c in [j, j + 1] {
            if c < est.len() {
                let d = est[c] - r;
                if d.abs() < best.abs() {
                    best = d;
                }
            }
        }
        out.push(best);
    }
    out
}

fn median(v: &mut Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

struct Row {
    id: u64,
    label: String,
    ref_bpm: f64,
    est_bpm: f64,
    raw_bpm: f64,
    ref_constant: bool,
    f70: f64,
    f70_raw: f64,
    med_offset_ms: f64,
    max_dev_ms: f64,
    p95_dev_ms: f64,
    fit_ms: f64,
    model_s: f64,
    duration: f64,
}

fn load_reference(path: &PathBuf) -> Vec<Reference> {
    let doc: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let mut out = Vec::new();
    for (k, v) in doc.as_object().unwrap() {
        let beats: Vec<f64> = v["beats_ms"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let n = beats.len();
        let constant = n > 2 && {
            let slope = (beats[n - 1] - beats[0]) / (n - 1) as f64;
            (0..n).all(|i| (beats[i] - (beats[0] + slope * i as f64)).abs() <= 5.0)
        };
        out.push(Reference {
            id: k.parse().unwrap(),
            artist: v["artist"].as_str().unwrap_or("").to_string(),
            title: v["title"].as_str().unwrap_or("").to_string(),
            bpm: v["bpm_masterdb"].as_f64().unwrap(),
            beats_ms: beats,
            constant,
        });
    }
    out.sort_by_key(|r| r.id);
    out
}

fn evaluate(refs: &[&Reference], dir: &PathBuf, params: &Params) -> Vec<Row> {
    let mut rows = Vec::new();
    for r in refs {
        let jp = dir.join(format!("{}.json", r.id));
        let lp = dir.join(format!("{}.logits", r.id));
        let Ok(text) = fs::read_to_string(&jp) else { continue };
        let doc: Value = serde_json::from_str(&text).unwrap();
        let raw = fs::read(&lp).unwrap();
        let all: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let frames = doc["frames"].as_u64().unwrap() as usize;
        let (bl, dl) = (&all[..frames], &all[frames..]);
        let peaks: Vec<f64> = doc["beats"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let duration = r.beats_ms.last().copied().unwrap_or(0.0).max(doc["duration_s"].as_f64().unwrap() * 1000.0) / 1000.0;
        let w = Window { offset: doc["offset_s"].as_f64().unwrap_or(0.0), len: frames as f64 / FPS };
        let Some(fit) = fit_track(&peaks, bl, dl, doc["duration_s"].as_f64().unwrap().max(duration), &w, params) else {
            continue;
        };
        let est_ms: Vec<f64> = fit.beats_s.iter().map(|t| t * 1000.0).collect();
        let raw_ms: Vec<f64> = fit.raw_beats_s.iter().map(|t| t * 1000.0).collect();
        let (_, f70) = match_f(&r.beats_ms, &est_ms, 70.0);
        let (_, f70_raw) = match_f(&r.beats_ms, &raw_ms, 70.0);
        let offs = nearest_offsets(&r.beats_ms, &est_ms);
        let mut abs: Vec<f64> = offs.iter().map(|d| d.abs()).collect();
        let mut signed = offs.clone();
        let med = median(&mut signed);
        let max_dev = abs.iter().cloned().fold(0.0, f64::max);
        abs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = abs[((abs.len() as f64) * 0.95) as usize % abs.len()];
        rows.push(Row {
            id: r.id,
            label: format!("{} - {}", r.artist, r.title),
            ref_bpm: r.bpm,
            est_bpm: fit.bpm,
            raw_bpm: fit.raw_bpm,
            ref_constant: r.constant,
            f70,
            f70_raw,
            med_offset_ms: med,
            max_dev_ms: max_dev,
            p95_dev_ms: p95,
            fit_ms: fit.fit_us as f64 / 1000.0,
            model_s: (doc["ms"]["model"].as_f64().unwrap() + doc["ms"]["decode"].as_f64().unwrap()) / 1000.0,
            duration: doc["duration_s"].as_f64().unwrap(),
        });
        let _ = fit.first_s;
    }
    rows
}

fn ratio_class(est: f64, r: f64) -> &'static str {
    let q = est / r;
    let near = |x: f64| (q - x).abs() < 0.02 * x;
    if (est - r).abs() <= 1.0 {
        "ok"
    } else if near(2.0) {
        "x2"
    } else if near(0.5) {
        "x0.5"
    } else if near(1.5) {
        "x1.5"
    } else if near(2.0 / 3.0) {
        "x2/3"
    } else {
        "other"
    }
}

fn summarize(name: &str, rows: &[&Row], use_raw_bpm: bool) -> Value {
    let n = rows.len() as f64;
    let pct = |c: usize| if n > 0.0 { 100.0 * c as f64 / n } else { 0.0 };
    let bpm = |r: &&Row| if use_raw_bpm { r.raw_bpm } else { r.est_bpm };
    let within = |tol: f64| rows.iter().filter(|r| (bpm(r) - r.ref_bpm).abs() <= tol).count();
    let count = |cls: &str| rows.iter().filter(|r| ratio_class(bpm(r), r.ref_bpm) == cls).count();
    let mut f: Vec<f64> = rows.iter().map(|r| r.f70).collect();
    let mut fr: Vec<f64> = rows.iter().map(|r| r.f70_raw).collect();
    let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let mut offs: Vec<f64> = rows.iter().map(|r| r.med_offset_ms).collect();
    let mut absoffs: Vec<f64> = rows.iter().map(|r| r.med_offset_ms.abs()).collect();
    let ok_bpm: Vec<&&Row> = rows.iter().filter(|r| ratio_class(bpm(r), r.ref_bpm) == "ok").collect();
    let _ = ok_bpm;
    let g20 = rows.iter().filter(|r| r.max_dev_ms <= 20.0).count();
    let p95_20 = rows.iter().filter(|r| r.p95_dev_ms <= 20.0).count();
    let g20_bpm_ok = rows.iter().filter(|r| r.max_dev_ms <= 20.0 && (r.est_bpm - r.ref_bpm).abs() <= 1.0).count();
    let _ = (&mut f, &mut fr);
    json!({
        "name": name,
        "n": rows.len(),
        "bpm_within_0.1": [within(0.1), pct(within(0.1))],
        "bpm_within_0.5": [within(0.5), pct(within(0.5))],
        "bpm_within_1": [within(1.0), pct(within(1.0))],
        "octave_x2": count("x2"), "octave_x0.5": count("x0.5"), "octave_x1.5": count("x1.5"), "octave_x2/3": count("x2/3"), "other_miss": count("other"),
        "f70_mean_fitted": mean(&f), "f70_mean_raw_peaks": mean(&fr),
        "median_offset_ms": median(&mut offs), "median_abs_offset_ms": median(&mut absoffs),
        "grid_within_20ms_all_beats": [g20, pct(g20)],
        "grid_p95_within_20ms": [p95_20, pct(p95_20)],
        "grid_within_20ms_and_bpm_within_1": [g20_bpm_ok, pct(g20_bpm_ok)],
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let reference = PathBuf::from(args.next().expect("usage: grid <reference.json> <dump dir> [--split dev|test|all] [--lo N] [--shift-ms N] [--comb] [--out file] [--dedupe]"));
    let dir = PathBuf::from(args.next().expect("dump dir"));
    let mut params = Params { lo: 70.0, shift_ms: 0.0, comb_refine: false };
    let mut split = "test".to_string();
    let mut out: Option<PathBuf> = None;
    let mut dedupe = false;
    let mut sweep = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--split" => split = args.next().unwrap(),
            "--lo" => params.lo = args.next().unwrap().parse().unwrap(),
            "--shift-ms" => params.shift_ms = args.next().unwrap().parse().unwrap(),
            "--comb" => params.comb_refine = true,
            "--out" => out = Some(PathBuf::from(args.next().unwrap())),
            "--dedupe" => dedupe = true,
            "--sweep" => sweep = true,
            other => panic!("unknown argument {other}"),
        }
    }
    let refs = load_reference(&reference);
    let mut seen = HashSet::new();
    let selected: Vec<&Reference> = refs
        .iter()
        .filter(|r| match split.as_str() {
            "test" => r.id % 3 == 0,
            "dev" => r.id % 3 != 0,
            _ => true,
        })
        .filter(|r| !dedupe || seen.insert(format!("{}|{}|{}", r.artist, r.title, r.bpm)))
        .collect();

    if sweep {
        for lo in [60.0, 65.0, 70.0, 75.0, 80.0, 85.0, 90.0, 95.0, 100.0] {
            let p = Params { lo, shift_ms: params.shift_ms, comb_refine: params.comb_refine };
            let rows = evaluate(&selected, &dir, &p);
            let rr: Vec<&Row> = rows.iter().collect();
            let s = summarize("sweep", &rr, false);
            println!("lo {lo}: n {} within1 {:?} within0.1 {:?} octave x2 {} x0.5 {} x1.5 {} x2/3 {} other {}", s["n"], s["bpm_within_1"], s["bpm_within_0.1"], s["octave_x2"], s["octave_x0.5"], s["octave_x1.5"], s["octave_x2/3"], s["other_miss"]);
        }
        return;
    }

    let rows = evaluate(&selected, &dir, &params);
    let all: Vec<&Row> = rows.iter().collect();
    let constant: Vec<&Row> = rows.iter().filter(|r| r.ref_constant).collect();
    let summaries = json!([
        summarize("all reference grids, fitted", &all, false),
        summarize("constant-tempo reference grids only, fitted", &constant, false),
        summarize("all reference grids, raw median-IBI BPM", &all, true),
    ]);
    let mut fit_ms: Vec<f64> = rows.iter().map(|r| r.fit_ms).collect();
    let mut model_s: Vec<f64> = rows.iter().map(|r| r.model_s).collect();
    let rt: Vec<f64> = rows.iter().map(|r| r.model_s / r.duration).collect();
    let worst: Vec<Value> = {
        let mut w: Vec<&Row> = rows.iter().filter(|r| ratio_class(r.est_bpm, r.ref_bpm) != "ok").collect();
        w.sort_by(|a, b| (b.est_bpm / b.ref_bpm - 1.0).abs().partial_cmp(&(a.est_bpm / a.ref_bpm - 1.0).abs()).unwrap());
        w.iter().take(25).map(|r| json!({"id": r.id, "track": r.label, "est": r.est_bpm, "ref": r.ref_bpm, "class": ratio_class(r.est_bpm, r.ref_bpm)})).collect()
    };
    let report = json!({
        "split": split, "lo": params.lo, "shift_ms": params.shift_ms, "comb": params.comb_refine, "dedupe": dedupe,
        "tracks_selected": selected.len(), "tracks_scored": rows.len(),
        "summaries": summaries,
        "fit_ms_median": median(&mut fit_ms),
        "model_plus_decode_s_median": median(&mut model_s),
        "model_s_per_audio_s_mean": rt.iter().sum::<f64>() / rt.len().max(1) as f64,
        "worst_bpm": worst,
    });
    let text = serde_json::to_string_pretty(&report).unwrap();
    println!("{text}");
    if let Some(o) = out {
        let per: Vec<Value> = rows
            .iter()
            .map(|r| json!({"id": r.id, "ref_bpm": r.ref_bpm, "est_bpm": r.est_bpm, "f70": r.f70, "med_offset_ms": r.med_offset_ms, "max_dev_ms": r.max_dev_ms, "ref_constant": r.ref_constant}))
            .collect();
        fs::write(o, serde_json::to_string(&json!({"report": report, "tracks": per})).unwrap()).unwrap();
    }
}
