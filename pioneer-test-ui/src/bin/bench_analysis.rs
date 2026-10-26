#[allow(dead_code)]
#[path = "../analysis_backend.rs"]
mod analysis_backend;
#[allow(dead_code)]
#[path = "../analysis_python.rs"]
mod analysis_python;
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;
#[allow(dead_code)]
#[path = "../analysis_run.rs"]
mod analysis_run;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use analysis_backend::{AnalysisBackend, BackendEnv};
use analysis_run::{AnalysisCoordinator, AnalysisSink, LogLevel, RunOutcome};
use pioneer_library::LocalLibrary;
use pioneer_usb_writer::scanner;

struct BenchSink {
    started: Instant,
    first_result_ms: Mutex<Option<f64>>,
    partial_ms: Mutex<HashMap<i64, f64>>,
    done_ms: Mutex<HashMap<i64, f64>>,
    log: Mutex<Vec<String>>,
    verbose: bool,
}

impl AnalysisSink for BenchSink {
    fn log(&self, level: LogLevel, message: String) {
        if self.verbose || matches!(level, LogLevel::Err) || message.starts_with("analysis backend") || message.starts_with("Starting") {
            eprintln!("[{}] {message}", level.as_str());
        }
        self.log.lock().unwrap().push(message);
    }

    fn progress(&self, _current: u32, _total: u32, _message: String, track_id: i64) {
        let now_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        self.first_result_ms.lock().unwrap().get_or_insert(now_ms);
        self.done_ms.lock().unwrap().insert(track_id, now_ms);
    }

    fn partial(&self, track_id: i64) {
        self.partial_ms.lock().unwrap().insert(track_id, self.started.elapsed().as_secs_f64() * 1000.0);
    }

    fn complete(&self) {}
}

fn process_tree_rss_mb(root: u32) -> f64 {
    let mut parent_of: HashMap<u32, u32> = HashMap::new();
    let mut rss_kb: HashMap<u32, f64> = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return 0.0;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(status) = std::fs::read_to_string(entry.path().join("status")) else {
            continue;
        };
        let field = |name: &str| {
            status
                .lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<f64>().ok())
        };
        if let Some(ppid) = field("PPid:") {
            parent_of.insert(pid, ppid as u32);
        }
        rss_kb.insert(pid, field("VmRSS:").unwrap_or(0.0));
    }
    let mut total = 0.0;
    for (&pid, &kb) in &rss_kb {
        let mut cursor = pid;
        for _ in 0..8 {
            if cursor == root {
                total += kb;
                break;
            }
            match parent_of.get(&cursor) {
                Some(&p) if p != 0 => cursor = p,
                _ => break,
            }
        }
    }
    total / 1024.0
}

fn main() {
    let mut backend = AnalysisBackend::Rust;
    let mut list: Option<PathBuf> = None;
    let mut limit = 50usize;
    let mut threads: Option<usize> = None;
    let mut verbose = false;
    let mut play_after_s: Option<f64> = None;
    let mut play_index = 0usize;
    let mut legacy_priority = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--backend" => backend = AnalysisBackend::parse(&it.next().unwrap()).expect("rust|python"),
            "--list" => list = Some(PathBuf::from(it.next().unwrap())),
            "--limit" => limit = it.next().unwrap().parse().unwrap(),
            "--threads" => threads = Some(it.next().unwrap().parse().unwrap()),
            "--verbose" => verbose = true,
            "--play-after" => play_after_s = Some(it.next().unwrap().parse().unwrap()),
            "--play-index" => play_index = it.next().unwrap().parse().unwrap(),
            "--legacy-priority" => legacy_priority = true,
            other => panic!("unknown argument {other}"),
        }
    }
    let paths: Vec<PathBuf> = std::fs::read_to_string(list.expect("--list <file of paths>"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(limit)
        .map(PathBuf::from)
        .collect();

    let dir = std::env::temp_dir().join(format!("bench-analysis-{}-{}", backend.name(), std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let library = LocalLibrary::open(&dir.join("library.db")).expect("open library");
    let tracks = scanner::scan_files(&paths).expect("scan");
    let ids = library.add_tracks(&tracks).expect("add tracks");
    let pending: Vec<(i64, PathBuf)> = ids.iter().zip(&tracks).map(|(id, t)| (*id, t.source_path.clone())).collect();
    let played = play_after_s.map(|_| pending[play_index].clone());
    let shared = Arc::new(Mutex::new(library));

    let env = BackendEnv { override_backend: Some(backend), max_threads: threads, ..BackendEnv::default() };
    let coordinator = AnalysisCoordinator::default();
    let started = Instant::now();
    let sink = Arc::new(BenchSink { started, first_result_ms: Mutex::new(None), partial_ms: Mutex::new(HashMap::new()), done_ms: Mutex::new(HashMap::new()), log: Mutex::new(Vec::new()), verbose });

    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(Mutex::new(0.0f64));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let rss = process_tree_rss_mb(std::process::id());
                let mut p = peak.lock().unwrap();
                *p = p.max(rss);
                drop(p);
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };

    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let outcome = runtime
        .block_on(async {
            let run = analysis_run::run_track_analysis(shared.clone(), coordinator.clone(), pending, sink.clone(), env.clone());
            let play = async {
                let (Some(delay), Some(item)) = (play_after_s, played.clone()) else { return };
                tokio::time::sleep(Duration::from_secs_f64(delay)).await;
                eprintln!("[bench] play track {} at {:.1}s", item.0, started.elapsed().as_secs_f64());
                if legacy_priority {
                    analysis_run::merge_into_analysis_queue(&coordinator, vec![item], true);
                } else {
                    analysis_run::prioritize_track(shared.clone(), coordinator.clone(), item, sink.clone(), env.clone()).await.expect("prioritize");
                }
            };
            let (outcome, ()) = tokio::join!(run, play);
            outcome
        })
        .expect("run");
    let wall_s = started.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    sampler.join().unwrap();

    let (errors, total) = match outcome {
        RunOutcome::Finished { errors, total } => (errors, total),
        _ => (0, 0),
    };
    let done = total - errors;
    let first = sink.first_result_ms.lock().unwrap().unwrap_or(0.0);
    println!(
        "BENCH backend={} tracks={} errors={errors} wall={wall_s:.1}s tracks_per_min={:.2} first_result={:.1}s peak_rss_tree={:.0}MB",
        backend.name(),
        done,
        done as f64 / wall_s * 60.0,
        first / 1000.0,
        *peak.lock().unwrap()
    );
    if let (Some(delay), Some((id, _))) = (play_after_s, played) {
        let at = |map: &Mutex<HashMap<i64, f64>>| map.lock().unwrap().get(&id).map(|ms| (ms / 1000.0 - delay).max(0.0));
        let waveform = at(&sink.partial_ms).or_else(|| at(&sink.done_ms));
        println!(
            "PLAY id={id} legacy={legacy_priority} to_waveform={:.1}s to_full={:.1}s",
            waveform.unwrap_or(f64::NAN),
            at(&sink.done_ms).unwrap_or(f64::NAN)
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
