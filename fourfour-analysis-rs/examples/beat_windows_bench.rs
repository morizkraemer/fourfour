use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use fourfour_analysis::analyze::{analyze_controlled, Config, Control};
use fourfour_analysis::bt::BeatEngine;
use fourfour_analysis::service::{beat_model, locate_models, prepacks_weights};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let list = args.next().expect("usage: beat_windows_bench <id<TAB>path list> [workers] [threads]");
    let workers: usize = args.next().map_or(1, |v| v.parse().unwrap());
    let threads: usize = args.next().map_or(1, |v| v.parse().unwrap());
    let items: Vec<(u64, PathBuf)> = std::fs::read_to_string(list)?
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, path)| (id.parse().unwrap(), PathBuf::from(path)))
        .collect();
    let models = locate_models(None).expect("model weights (FOURFOUR_MODELS_DIR)");
    let beat_file = beat_model();
    let engine = BeatEngine::load(&models, &beat_file, prepacks_weights(&beat_file))?;
    engine.share_chunk_threads(workers * threads);
    let cfg = Config { threads, key: false, waveform: false, ..Config::default() };
    eprintln!("beat model {beat_file}, windows {:?}, workers {workers}, threads {threads}", cfg.beat_windows);
    let next = AtomicUsize::new(0);
    let out = Mutex::new(std::io::stdout());
    let cancel = AtomicBool::new(false);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((id, path)) = items.get(i) else { break };
                let line = match analyze_controlled(&engine, path, &cfg, &Control::new(&cancel)) {
                    Ok(a) => a.to_json(path, Some(*id)),
                    Err(e) => serde_json::json!({ "id": id, "path": path, "error": e.to_string() }),
                };
                let mut out = out.lock().unwrap();
                writeln!(out, "{line}").unwrap();
                eprintln!("[{}/{}] {}", i + 1, items.len(), path.display());
            });
        }
    });
    Ok(())
}
