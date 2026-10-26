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

    fn progress(&self, _current: u32, _total: u32, _message: String, _track_id: i64) {
        self.first_result_ms
            .lock()
            .unwrap()
            .get_or_insert(self.started.elapsed().as_secs_f64() * 1000.0);
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
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--backend" => backend = AnalysisBackend::parse(&it.next().unwrap()).expect("rust|python"),
            "--list" => list = Some(PathBuf::from(it.next().unwrap())),
            "--limit" => limit = it.next().unwrap().parse().unwrap(),
            "--threads" => threads = Some(it.next().unwrap().parse().unwrap()),
            "--verbose" => verbose = true,
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
    let shared = Arc::new(Mutex::new(library));

    let env = BackendEnv { override_backend: Some(backend), max_threads: threads, ..BackendEnv::default() };
    let coordinator = AnalysisCoordinator::default();
    let started = Instant::now();
    let sink = Arc::new(BenchSink { started, first_result_ms: Mutex::new(None), log: Mutex::new(Vec::new()), verbose });

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
        .block_on(analysis_run::run_track_analysis(shared.clone(), coordinator, pending, sink.clone(), env))
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
    std::fs::remove_dir_all(&dir).ok();
}
