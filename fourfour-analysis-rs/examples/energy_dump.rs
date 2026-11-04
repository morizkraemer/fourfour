use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use fourfour_analysis::analyze::{analyze_controlled, Config, Control};
use fourfour_analysis::bt::BeatEngine;
use fourfour_analysis::service::{locate_models, BEAT_MODEL};
use serde_json::json;

fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let after_comm = stat.rsplit(')').next().unwrap_or("");
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let ticks = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) / 100.0
}

fn main() -> anyhow::Result<()> {
    let mut energy = true;
    let mut full = true;
    let mut beats = true;
    let mut workers = 1usize;
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--energy" => energy = args.next().expect("--energy 0|1") == "1",
            "--lean" => full = false,
            "--no-beats" => beats = false,
            "--workers" => workers = args.next().expect("--workers N").parse()?,
            "--list" => {
                let text = std::fs::read_to_string(args.next().expect("--list file"))?;
                paths.extend(text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let dir = locate_models(None).ok_or_else(|| anyhow::anyhow!("models not found; set FOURFOUR_MODELS_DIR"))?;
    let engine = BeatEngine::load(&dir, BEAT_MODEL, false)?;
    engine.share_chunk_threads(workers);
    let cfg = Config { threads: 1, energy, beats, key: full, waveform: full, ..Config::default() };
    let next = AtomicUsize::new(0);
    let out = Mutex::new(std::io::stdout());
    let cancel = AtomicBool::new(false);
    let wall = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(i) else { break };
                let doc = match analyze_controlled(&engine, path, &cfg, &Control::new(&cancel)) {
                    Ok(a) => json!({
                        "path": path,
                        "duration_s": a.duration_s,
                        "bpm": a.bpm,
                        "ms": a.ms,
                        "energy": a.energy.as_ref().map(|e| json!({
                            "rating": e.rating,
                            "score": e.score,
                            "window_s": e.window_s,
                            "curve": e.curve,
                            "windows": e.windows,
                        })),
                    }),
                    Err(err) => json!({ "path": path, "error": err.to_string() }),
                };
                use std::io::Write;
                writeln!(out.lock().unwrap(), "{doc}").unwrap();
            });
        }
    });
    eprintln!("tracks {} wall {:.1} s cpu {:.1} s", paths.len(), wall.elapsed().as_secs_f64(), cpu_seconds());
    Ok(())
}
