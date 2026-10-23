use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use beat_this::{load_audio, BeatThis, RtenRuntime};
use serde_json::{json, Value};

struct Args {
    reference: PathBuf,
    out: PathBuf,
    shard: (usize, usize),
    limit: usize,
    models: PathBuf,
    small: bool,
    window_s: f64,
}

fn parse_args() -> Args {
    let mut a = Args {
        reference: PathBuf::new(),
        out: PathBuf::new(),
        shard: (0, 1),
        limit: usize::MAX,
        models: Path::new(env!("CARGO_MANIFEST_DIR")).join("models"),
        small: false,
        window_s: 0.0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--reference" => a.reference = PathBuf::from(it.next().expect("--reference path")),
            "--out" => a.out = PathBuf::from(it.next().expect("--out dir")),
            "--shard" => {
                let s = it.next().expect("--shard i/n");
                let (i, n) = s.split_once('/').expect("--shard i/n");
                a.shard = (i.parse().unwrap(), n.parse().unwrap());
            }
            "--small" => a.small = true,
            "--window" => a.window_s = it.next().expect("--window seconds").parse().unwrap(),
            "--limit" => a.limit = it.next().expect("--limit n").parse().unwrap(),
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn main() {
    let args = parse_args();
    fs::create_dir_all(&args.out).unwrap();
    let reference: Value = serde_json::from_str(&fs::read_to_string(&args.reference).unwrap()).unwrap();
    let mut jobs: Vec<(u64, String, PathBuf)> = reference
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, v)| v["fingerprint_verified"].as_bool() == Some(true))
        .map(|(k, v)| (k.parse().unwrap(), k.clone(), PathBuf::from(v["path"].as_str().unwrap())))
        .collect();
    let priority = |id: u64| if id % 12 == 4 { 0 } else if id % 3 == 0 { 1 } else if id % 12 == 7 { 2 } else { 3 };
    jobs.sort_by_key(|(id, _, _)| (priority(*id), *id));
    let jobs: Vec<_> = jobs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % args.shard.1 == args.shard.0)
        .map(|(_, j)| j)
        .take(args.limit)
        .collect();

    let beat_model = args.models.join(if args.small { "beat_this_small.onnx" } else { "beat_this.onnx" });
    let mut bt = BeatThis::new(&RtenRuntime, &args.models.join("mel_spectrogram.onnx"), &beat_model)
        .expect("load beat models");

    for (_, id, path) in jobs {
        let json_path = args.out.join(format!("{id}.json"));
        if json_path.exists() {
            continue;
        }
        let total = Instant::now();
        let t = Instant::now();
        let audio = match load_audio(&path, 22050) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{id}: decode failed: {e}");
                continue;
            }
        };
        let decode_ms = t.elapsed().as_secs_f64() * 1000.0;
        let duration_s = audio.samples.len() as f64 / 22050.0;
        let window_len = (args.window_s * 22050.0) as usize;
        let (samples, offset_s) = if args.window_s > 0.0 && audio.samples.len() > window_len {
            let start = (audio.samples.len() - window_len) / 2;
            (audio.samples[start..start + window_len].to_vec(), start as f64 / 22050.0)
        } else {
            (audio.samples, 0.0)
        };

        let t = Instant::now();
        let a = match bt.analyze_owned(samples, 22050) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{id}: beat model failed: {e}");
                continue;
            }
        };
        let model_ms = t.elapsed().as_secs_f64() * 1000.0;

        let mut logits: Vec<u8> = Vec::with_capacity(a.beat_logits.len() * 8);
        for v in a.beat_logits.iter().chain(a.downbeat_logits.iter()) {
            logits.extend_from_slice(&v.to_le_bytes());
        }
        fs::write(args.out.join(format!("{id}.logits")), logits).unwrap();

        let doc = json!({
            "id": id,
            "duration_s": duration_s,
            "offset_s": offset_s,
            "frames": a.beat_logits.len(),
            "ms": { "decode": decode_ms, "model": model_ms, "total": total.elapsed().as_secs_f64() * 1000.0 },
            "beats": a.beats.iter().map(|b| *b as f64 + offset_s).collect::<Vec<f64>>(),
            "downbeats": a.downbeats.iter().map(|b| *b as f64 + offset_s).collect::<Vec<f64>>(),
        });
        fs::write(&json_path, serde_json::to_string(&doc).unwrap()).unwrap();
        eprintln!("{id}: {:.0} s audio, decode {:.1} s, model {:.1} s", duration_s, decode_ms / 1000.0, model_ms / 1000.0);
    }
}
