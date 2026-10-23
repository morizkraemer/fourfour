use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use ffcore::analyze::{analyze, Config};
use ffcore::bt::BeatEngine;
use ffcore::mp3::DelayRule;
use serde_json::{json, Value};

struct Args {
    files: Vec<PathBuf>,
    models: PathBuf,
    beat_file: String,
    prepack: bool,
    threads: Option<usize>,
    workers: String,
    rss_mb_per_worker: f64,
    mem_fraction: f64,
    brief: bool,
    out: Option<PathBuf>,
    dump: Option<PathBuf>,
    reference: Option<PathBuf>,
    cfg: Config,
}

fn parse_args() -> Args {
    let mut a = Args {
        files: Vec::new(),
        models: Path::new(env!("CARGO_MANIFEST_DIR")).join("models"),
        beat_file: "beat_this.onnx".to_string(),
        prepack: false,
        threads: None,
        workers: "1".to_string(),
        rss_mb_per_worker: 900.0,
        mem_fraction: 0.5,
        brief: false,
        out: None,
        dump: None,
        reference: None,
        cfg: Config::default(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--models" => a.models = PathBuf::from(val()),
            "--small" => a.beat_file = "beat_this_small.onnx".to_string(),
            "--beat-model" => a.beat_file = val(),
            "--prepack" => a.prepack = true,
            "--threads" => a.threads = Some(val().parse().unwrap()),
            "--workers" => a.workers = val(),
            "--rss-mb-per-worker" => a.rss_mb_per_worker = val().parse().unwrap(),
            "--mem-fraction" => a.mem_fraction = val().parse().unwrap(),
            "--no-key" => a.cfg.key = false,
            "--no-waveform" => a.cfg.waveform = false,
            "--brief" => a.brief = true,
            "--out" => a.out = Some(PathBuf::from(val())),
            "--dump" => {
                a.dump = Some(PathBuf::from(val()));
                a.cfg.keep_logits = true;
            }
            "--reference" => a.reference = Some(PathBuf::from(val())),
            "--mp3-rule" => a.cfg.mp3_rule = DelayRule::parse(&val()).expect("rule none|lame|lame+header|header"),
            "--list" => {
                let text = std::fs::read_to_string(val()).unwrap();
                a.files.extend(text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from));
            }
            _ => a.files.push(PathBuf::from(arg)),
        }
    }
    a
}

fn mem_available_mb() -> f64 {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    text.lines()
        .find(|l| l.starts_with("MemAvailable:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(4096.0, |kb| kb / 1024.0)
}

fn peak_rss_mb() -> f64 {
    let text = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    text.lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

fn load_ids(reference: &Path) -> HashMap<String, u64> {
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(reference).unwrap()).unwrap();
    doc.as_object()
        .unwrap()
        .iter()
        .filter_map(|(k, v)| Some((v["path"].as_str()?.to_string(), k.parse().ok()?)))
        .collect()
}

fn dump(dir: &Path, id: u64, a: &ffcore::analyze::Analysis) {
    std::fs::create_dir_all(dir).unwrap();
    let (bl, dl) = a.logits.as_ref().unwrap();
    let mut bytes = Vec::with_capacity(bl.len() * 8);
    for v in bl.iter().chain(dl.iter()) {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(dir.join(format!("{id}.logits")), bytes).unwrap();
    let peaks: Vec<f64> = a.raw_beats_s.clone();
    let doc = json!({
        "id": id,
        "duration_s": a.duration_s,
        "offset_s": 0.0,
        "frames": bl.len(),
        "ms": { "decode": a.ms["decode"], "model": a.ms["model"], "total": a.ms["total"] },
        "beats": peaks,
        "downbeats": a.downbeats_s,
        "final_beats_s": a.beats_s,
        "bpm": a.bpm,
    });
    std::fs::write(dir.join(format!("{id}.json")), serde_json::to_string(&doc).unwrap()).unwrap();
}

fn main() {
    let args = parse_args();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let cap_by_mem = ((mem_available_mb() * args.mem_fraction) / args.rss_mb_per_worker).floor().max(1.0) as usize;
    let requested = if args.workers == "auto" { cores } else { args.workers.parse().expect("--workers N|auto") };
    let workers = requested.min(cap_by_mem).min(args.files.len().max(1));
    let threads = args.threads.unwrap_or((cores / workers).max(1));
    let ids = args.reference.as_deref().map(load_ids).unwrap_or_default();
    let mut cfg = args.cfg.clone();
    cfg.threads = threads;

    let wall = Instant::now();
    let t = Instant::now();
    let engine = BeatEngine::load(&args.models, &args.beat_file, args.prepack).expect("load beat models");
    let load_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("workers {workers} (requested {requested}, memory cap {cap_by_mem}) threads/track {threads}; model load {load_ms:.0} ms");

    let next = AtomicUsize::new(0);
    let sink: Mutex<Box<dyn Write + Send>> = Mutex::new(match &args.out {
        Some(p) => Box::new(std::fs::File::create(p).unwrap()),
        None => Box::new(std::io::stdout()),
    });
    let done = AtomicUsize::new(0);
    let first_done_ms = Mutex::new(None::<f64>);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= args.files.len() {
                    break;
                }
                let path = &args.files[i];
                let id = ids.get(path.to_string_lossy().as_ref()).copied();
                match analyze(&engine, path, &cfg) {
                    Ok(a) => {
                        if let (Some(dir), Some(id)) = (&args.dump, id) {
                            dump(dir, id, &a);
                        }
                        let mut doc = a.to_json(path, id);
                        if args.brief {
                            doc.as_object_mut().unwrap().remove("final_beats_s");
                            doc.as_object_mut().unwrap().remove("downbeats_s");
                        }
                        writeln!(sink.lock().unwrap(), "{}", serde_json::to_string(&doc).unwrap()).unwrap();
                    }
                    Err(e) => eprintln!("{}: {e}", path.display()),
                }
                done.fetch_add(1, Ordering::SeqCst);
                first_done_ms.lock().unwrap().get_or_insert(wall.elapsed().as_secs_f64() * 1000.0);
            });
        }
    });
    let wall_s = wall.elapsed().as_secs_f64();
    let n = done.load(Ordering::SeqCst);
    eprintln!(
        "SUMMARY tracks {n} wall {wall_s:.1} s = {:.2} tracks/min; model load {load_ms:.0} ms; first track done at {:.0} ms; peak RSS {:.0} MB; workers {workers} threads/track {threads}",
        n as f64 / wall_s * 60.0,
        first_done_ms.lock().unwrap().unwrap_or(0.0),
        peak_rss_mb()
    );
}
