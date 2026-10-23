mod chroma;
mod model;
mod spectro;
mod weights;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use chroma::{features, Chroma, Params, TrackFeatures, CHANNELS};
use model::*;
use spectro::Spec;

struct Entry {
    id: String,
    file: String,
    truth: usize,
}

fn load_manifest(path: &Path) -> Vec<Entry> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("read manifest")).unwrap();
    json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| Entry {
            id: e["id"].as_str().unwrap().to_string(),
            file: e["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string(),
            truth: class_from_camelot(e["ground_truth"]["key"].as_str().unwrap()).expect("camelot"),
        })
        .collect()
}

fn is_test(e: &Entry) -> bool {
    e.id.parse::<usize>().unwrap() % 3 == 0
}

fn parallel_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<R>>> = items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= items.len() {
                    break;
                }
                *results[i].lock().unwrap() = Some(f(&items[i]));
            });
        }
    });
    results.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

fn extract(manifest: &Path, audio_dir: &Path, cache_dir: &Path, threads: usize) {
    std::fs::create_dir_all(cache_dir).unwrap();
    let entries = load_manifest(manifest);
    let times = parallel_map(&entries, threads, |e| {
        let out = cache_dir.join(format!("{}.spec", e.id));
        if out.exists() {
            return 0.0;
        }
        let t = Instant::now();
        let audio = beat_this::load_audio(&audio_dir.join(&e.file), 44100).expect("decode");
        let spec = spectro::analyze(&audio.samples);
        std::fs::write(out, spec.to_bytes()).unwrap();
        t.elapsed().as_secs_f64()
    });
    let done: Vec<f64> = times.into_iter().filter(|t| *t > 0.0).collect();
    eprintln!("extracted {} tracks, mean {:.3} s/track", done.len(), done.iter().sum::<f64>() / done.len().max(1) as f64);
}

struct Opts(HashMap<String, String>);

impl Opts {
    fn get<T: std::str::FromStr>(&self, k: &str, default: T) -> T {
        self.0.get(k).map(|v| v.parse().unwrap_or_else(|_| panic!("bad value for {k}"))).unwrap_or(default)
    }
}

fn params_from(o: &Opts) -> Params {
    let d = Params::default();
    Params {
        hpss: o.get::<u8>("hpss", d.hpss as u8) == 1,
        hpss_time: o.get("hpss_time", d.hpss_time),
        hpss_freq: o.get("hpss_freq", d.hpss_freq),
        gamma: o.get("gamma", d.gamma),
        whiten: o.get("whiten", d.whiten),
        harmonics: o.get("harmonics", d.harmonics),
        harmonic_decay: o.get("harmonic_decay", d.harmonic_decay),
        segment_frames: o.get("seg", d.segment_frames),
        bass_hi: o.get("bass_hi", d.bass_hi),
        mid_lo: o.get("mid_lo", d.mid_lo),
        mid_hi: o.get("mid_hi", d.mid_hi),
    }
}

#[derive(Default)]
struct Tally {
    n: usize,
    exact: usize,
    relative: usize,
    parallel: usize,
    fifth: usize,
    other_mode: usize,
}

impl Tally {
    fn add(&mut self, pred: usize, truth: usize) {
        self.n += 1;
        let (pt, tt) = (tonic_of(pred), tonic_of(truth));
        let (pm, tm) = (is_minor(pred), is_minor(truth));
        if pred == truth {
            self.exact += 1;
        } else if pm != tm && pt == tt {
            self.parallel += 1;
        } else if pm != tm && ((!pm && pt == (tt + 3) % 12) || (pm && pt == (tt + 9) % 12)) {
            self.relative += 1;
        } else if pm == tm && (pt + 12 - tt) % 12 == 7 || pm == tm && (tt + 12 - pt) % 12 == 7 {
            self.fifth += 1;
        } else if pm != tm {
            self.other_mode += 1;
        }
    }

    fn report(&self, label: &str) {
        let p = |x: usize| 100.0 * x as f64 / self.n as f64;
        println!(
            "{label} n={} exact={:.1}% relative={:.1}% parallel={:.1}% fifth={:.1}% other_mode={:.1}% exact+rel+fifth={:.1}% wrong_mode={:.1}%",
            self.n,
            p(self.exact),
            p(self.relative),
            p(self.parallel),
            p(self.fifth),
            p(self.other_mode),
            p(self.exact + self.relative + self.fifth),
            p(self.relative + self.parallel + self.other_mode)
        );
    }
}

fn load_features(entries: &[Entry], cache: &Path, params: &Params, threads: usize) -> Vec<TrackFeatures> {
    parallel_map(entries, threads, |e| {
        let spec = Spec::from_bytes(&std::fs::read(cache.join(format!("{}.spec", e.id))).expect("run extract first"));
        features(&spec, params)
    })
}

fn mix(c: &Chroma, bass_weight: f32) -> [f32; 12] {
    let mut out = [0f32; 12];
    for i in 0..12 {
        out[i] = c[1][i] + bass_weight * c[0][i];
    }
    out
}

fn profiles(name: &str) -> ([f32; 12], [f32; 12]) {
    match name {
        "temperley" => (TEMPERLEY_MAJOR, TEMPERLEY_MINOR),
        _ => (KRUMHANSL_MAJOR, KRUMHANSL_MINOR),
    }
}

fn predict_linear(model: &Linear, f: &TrackFeatures, vote: &str) -> usize {
    if vote == "mean" || f.segments.is_empty() {
        return argmax(&model.logits(&f.mean));
    }
    let mut total = [0f32; CLASSES];
    for s in &f.segments {
        let lp = log_softmax(&model.logits(s));
        for i in 0..CLASSES {
            total[i] += lp[i];
        }
    }
    argmax(&total)
}

fn train_samples(entries: &[&Entry], feats: &[&TrackFeatures], use_segments: bool) -> Vec<Sample> {
    let mut out = Vec::new();
    for (e, f) in entries.iter().zip(feats) {
        if use_segments && !f.segments.is_empty() {
            out.extend(f.segments.iter().map(|s| Sample { chroma: *s, label: e.truth }));
        } else {
            out.push(Sample { chroma: f.mean, label: e.truth });
        }
    }
    out
}

fn eval(manifest: &Path, cache: &Path, o: &Opts) {
    let params = params_from(o);
    let threads = o.get("threads", 4usize);
    let entries = load_manifest(manifest);
    let feats = load_features(&entries, cache, &params, threads);
    let method: String = o.get("method", "linear".to_string());
    let split: String = o.get("split", "all".to_string());
    let vote: String = o.get("vote", "mean".to_string());
    let label = format!("{method} split={split} {:?}", o.0);

    let mut tally = Tally::default();
    if method == "template" {
        let (maj, min) = profiles(&o.get("profile", "temperley".to_string()));
        let bass_weight = o.get("bassw", 0.0f32);
        for (e, f) in entries.iter().zip(&feats) {
            let sel = match split.as_str() {
                "test" => is_test(e),
                "train" => !is_test(e),
                _ => true,
            };
            if !sel {
                continue;
            }
            let pred = if vote == "mean" || f.segments.is_empty() {
                argmax(&template_scores(&mix(&f.mean, bass_weight), &maj, &min))
            } else {
                let mut total = [0f32; CLASSES];
                for s in &f.segments {
                    let sc = template_scores(&mix(s, bass_weight), &maj, &min);
                    let w = argmax(&sc);
                    total[w] += 1.0;
                }
                argmax(&total)
            };
            tally.add(pred, e.truth);
        }
    } else if method == "shipped" {
        let m = Linear { weights: weights::WEIGHTS, bias: weights::BIAS };
        for (e, f) in entries.iter().zip(&feats).filter(|(e, _)| split == "all" || is_test(e)) {
            tally.add(predict_linear(&m, f, &vote), e.truth);
        }
    } else {
        let l2 = o.get("l2", 0.001f32);
        let epochs = o.get("epochs", 300usize);
        let lr = o.get("lr", 0.05f32);
        let seg_train = o.get::<u8>("segtrain", 0) == 1;
        let idx_train: Vec<usize> = (0..entries.len()).filter(|i| split == "cvall" || !is_test(&entries[*i])).collect();
        let fit_on = |train: &[usize]| {
            let es: Vec<&Entry> = train.iter().map(|i| &entries[*i]).collect();
            let fs: Vec<&TrackFeatures> = train.iter().map(|i| &feats[*i]).collect();
            fit(&train_samples(&es, &fs, seg_train), l2, epochs, lr)
        };
        match split.as_str() {
            "test" => {
                let m = fit_on(&idx_train);
                for (e, f) in entries.iter().zip(&feats).filter(|(e, _)| is_test(e)) {
                    tally.add(predict_linear(&m, f, &vote), e.truth);
                }
            }
            _ => {
                let k = o.get("folds", 5usize);
                let folds: Vec<Vec<usize>> = (0..k).map(|f| idx_train.iter().cloned().enumerate().filter(|(j, _)| j % k == f).map(|(_, i)| i).collect()).collect();
                let results = parallel_map(&folds, k.min(threads), |held| {
                    let train: Vec<usize> = idx_train.iter().cloned().filter(|i| !held.contains(i)).collect();
                    let m = fit_on(&train);
                    held.iter().map(|i| (predict_linear(&m, &feats[*i], &vote), entries[*i].truth)).collect::<Vec<_>>()
                });
                for fold in results {
                    for (p, t) in fold {
                        tally.add(p, t);
                    }
                }
            }
        }
    }
    tally.report(&label);
}

fn export(manifest: &Path, cache: &Path, out: &Path, o: &Opts) {
    let params = params_from(o);
    let entries = load_manifest(manifest);
    let feats = load_features(&entries, cache, &params, o.get("threads", 4usize));
    let only_train = o.get::<String>("split", "train".to_string()) == "train";
    let idx: Vec<usize> = (0..entries.len()).filter(|i| !only_train || !is_test(&entries[*i])).collect();
    let es: Vec<&Entry> = idx.iter().map(|i| &entries[*i]).collect();
    let fs: Vec<&TrackFeatures> = idx.iter().map(|i| &feats[*i]).collect();
    let m = fit(&train_samples(&es, &fs, false), o.get("l2", 0.001f32), o.get("epochs", 300usize), o.get("lr", 0.05f32));
    let mut text = format!("pub const TRAINED_ON_TRACKS: usize = {};\npub const WEIGHTS: [[[f32; 12]; 2]; 2] = [\n", idx.len());
    for mode in 0..2 {
        text.push_str("    [\n");
        for d in 0..CHANNELS {
            text.push_str(&format!("        {:?},\n", m.weights[mode][d]));
        }
        text.push_str("    ],\n");
    }
    text.push_str(&format!("];\npub const BIAS: [f32; 2] = {:?};\n", m.bias));
    std::fs::write(out, text).unwrap();
}

fn detect(files: &[PathBuf]) {
    let model = Linear { weights: weights::WEIGHTS, bias: weights::BIAS };
    let params = Params::default();
    for f in files {
        let t0 = Instant::now();
        let audio = beat_this::load_audio(f, 44100).expect("decode");
        let t1 = Instant::now();
        let spec = spectro::analyze(&audio.samples);
        let t2 = Instant::now();
        let feats = features(&spec, &params);
        let class = argmax(&model.logits(&feats.mean));
        let t3 = Instant::now();
        println!(
            "{{\"path\":{:?},\"camelot\":\"{}\",\"ms\":{{\"decode\":{:.0},\"spectrum\":{:.0},\"classify\":{:.0}}}}}",
            f,
            camelot(class),
            (t1 - t0).as_secs_f64() * 1e3,
            (t2 - t1).as_secs_f64() * 1e3,
            (t3 - t2).as_secs_f64() * 1e3
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args.split_first().expect("usage: keydet <extract|eval> ...");
    let positional: Vec<&String> = rest.iter().filter(|a| !a.contains('=')).collect();
    let opts = Opts(rest.iter().filter_map(|a| a.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect());
    match cmd.as_str() {
        "extract" => extract(
            &PathBuf::from(positional[0]),
            &PathBuf::from(positional[1]),
            &PathBuf::from(positional[2]),
            opts.get("threads", 4usize),
        ),
        "export" => export(&PathBuf::from(positional[0]), &PathBuf::from(positional[1]), &PathBuf::from(positional[2]), &opts),
        "detect" => detect(&positional.iter().map(|p| PathBuf::from(p)).collect::<Vec<_>>()),
        "eval" => eval(&PathBuf::from(positional[0]), &PathBuf::from(positional[1]), &opts),
        other => panic!("unknown command {other}"),
    }
    let _ = CHANNELS;
}
