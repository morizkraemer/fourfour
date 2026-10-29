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
#[path = "../analysis_priority.rs"]
mod analysis_priority;
#[allow(dead_code)]
#[path = "../waveform_detail.rs"]
mod waveform_detail;

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
    partial_ms: Mutex<HashMap<i64, Vec<f64>>>,
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
        self.partial_ms.lock().unwrap().entry(track_id).or_default().push(self.started.elapsed().as_secs_f64() * 1000.0);
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

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.get(v.len() / 2).copied().unwrap_or(f64::NAN)
}

/// One track at a time through the analysis crate directly, to read its per-stage timings
/// (`Analysis::ms`), then the library write the app does with the result. Key and waveform run
/// in parallel with the beat model, so the stages add up to more than `total`.
fn profile_stages(library: &LocalLibrary, items: &[(i64, PathBuf)], threads: Option<usize>, dump: Option<&PathBuf>) {
    use fourfour_analysis::analyze::{analyze_controlled, Config, Control};
    use fourfour_analysis::bt::BeatEngine;
    use fourfour_analysis::service::{locate_models, plan_pool, available_memory_mb, to_app_result, BEAT_MODEL};

    let models = locate_models(None).expect("model weights (FOURFOUR_MODELS_DIR)");
    let t = Instant::now();
    let engine = BeatEngine::load(&models, BEAT_MODEL, false).expect("load models");
    let model_load_ms = t.elapsed().as_secs_f64() * 1000.0;
    let env = BackendEnv { max_threads: threads, ..BackendEnv::default() };
    let threads = plan_pool(&env.pool_options(), available_memory_mb(), 1).threads_per_track;
    let cfg = Config { threads, keep_logits: dump.is_some(), ..Config::default() };
    let mut dumped = Vec::new();
    let stages = ["decode", "resample", "mel", "predict", "fit", "mp3", "key", "waveform", "total"];
    let mut per_stage: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut per_audio_s: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut writes = Vec::new();
    let mut to_result = Vec::new();
    let mut audio_s = 0.0;
    let cancel = AtomicBool::new(false);
    for (id, path) in items {
        let analysis = analyze_controlled(&engine, path, &cfg, &Control::new(&cancel)).expect("analyze");
        let t = Instant::now();
        let result = to_app_result(&analysis);
        to_result.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        library.set_analysis(*id, &result).expect("set_analysis");
        writes.push(t.elapsed().as_secs_f64() * 1000.0);
        audio_s += analysis.duration_s;
        for stage in stages {
            let v = analysis.ms[stage].as_f64().unwrap_or(0.0);
            per_stage.entry(stage).or_default().push(v);
            per_audio_s.entry(stage).or_default().push(v / analysis.duration_s);
        }
        eprintln!("[profile] {} {:.0}s audio {}", path.display(), analysis.duration_s, analysis.ms);
        if dump.is_some() {
            let mut doc = analysis.to_json(path, Some(*id as u64));
            doc["logits"] = serde_json::json!(analysis.logits);
            doc["result"] = serde_json::json!(result);
            dumped.push(doc);
        }
    }
    if let Some(file) = dump {
        std::fs::write(file, serde_json::to_string(&dumped).unwrap()).expect("write dump");
    }
    let mut values = serde_json::Map::new();
    values.insert("tracks".into(), items.len().into());
    values.insert("threads_per_track".into(), threads.into());
    values.insert("audio_s_total".into(), audio_s.into());
    values.insert("model_load_ms".into(), model_load_ms.into());
    let mut line = format!("PROFILE tracks={} threads={threads} model_load={model_load_ms:.0}ms", items.len());
    for stage in stages.iter().copied().chain(["to_app_result", "write"]) {
        let ms = match stage {
            "to_app_result" => to_result.clone(),
            "write" => writes.clone(),
            s => per_stage[s].clone(),
        };
        let sum: f64 = ms.iter().sum();
        let med = median(ms);
        let per_min = if stage == "write" || stage == "to_app_result" { sum / audio_s * 60.0 } else { median(per_audio_s[stage].clone()) * 60.0 };
        values.insert(format!("{stage}_ms_median"), med.into());
        values.insert(format!("{stage}_ms_per_audio_min"), per_min.into());
        line.push_str(&format!(" {stage}={med:.0}ms({per_min:.0}ms/min)"));
    }
    println!("{line}");
    println!("PERF {}", serde_json::json!({ "bench": "analysis_profile", "values": values }));
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
    let mut unstaged = false;
    let mut profile = 0usize;
    let mut dump: Option<PathBuf> = None;
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
            "--unstaged" => unstaged = true,
            "--profile" => profile = it.next().unwrap().parse().unwrap(),
            "--dump" => dump = Some(PathBuf::from(it.next().unwrap())),
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
    if profile > 0 {
        profile_stages(&library, &pending[..profile.min(pending.len())], threads, dump.as_ref());
        std::fs::remove_dir_all(&dir).ok();
        return;
    }
    let shared = Arc::new(Mutex::new(library));

    let env = BackendEnv { override_backend: Some(backend), max_threads: threads, ..BackendEnv::default() };
    let coordinator = AnalysisCoordinator::default().with_staging(!unstaged);
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
    println!(
        "PERF {}",
        serde_json::json!({ "bench": "analysis_throughput", "values": {
            "backend": backend.name(), "staged": !unstaged, "tracks": done, "errors": errors,
            "wall_s": wall_s, "tracks_per_min": done as f64 / wall_s * 60.0,
            "first_result_s": first / 1000.0, "peak_rss_tree_mb": *peak.lock().unwrap(),
        }})
    );
    if let (Some(delay), Some((id, _))) = (play_after_s, played) {
        let rel = |ms: f64| (ms / 1000.0 - delay).max(0.0);
        let first_partial = sink.partial_ms.lock().unwrap().get(&id).and_then(|v| v.first().copied()).map(rel);
        let done = sink.done_ms.lock().unwrap().get(&id).copied().map(rel);
        println!(
            "PLAY id={id} legacy={legacy_priority} to_waveform={:.1}s to_full={:.1}s",
            first_partial.or(done).unwrap_or(f64::NAN),
            done.unwrap_or(f64::NAN)
        );
    }
    {
        let partials = sink.partial_ms.lock().unwrap();
        let done = sink.done_ms.lock().unwrap();
        let latest = |pick: &dyn Fn(&[f64]) -> Option<f64>| {
            ids.iter().filter_map(|id| pick(partials.get(id).map_or(&[][..], |v| &v[..])).or_else(|| done.get(id).copied())).fold(0.0f64, f64::max) / 1000.0
        };
        println!(
            "STAGES staged={} all_waveform={:.1}s all_key={:.1}s all_grid={:.1}s",
            !unstaged,
            latest(&|v| v.first().copied()),
            latest(&|v| v.get(1).copied()),
            ids.iter().filter_map(|id| done.get(id).copied()).fold(0.0f64, f64::max) / 1000.0
        );
    }
    if let Some(file) = dump {
        let lib = shared.lock().unwrap();
        let stored: Vec<serde_json::Value> = ids
            .iter()
            .zip(&paths)
            .map(|(id, path)| serde_json::json!({ "id": id, "path": path, "result": lib.get_analysis(*id).ok().flatten() }))
            .collect();
        std::fs::write(file, serde_json::to_string(&stored).unwrap()).expect("write dump");
    }
    std::fs::remove_dir_all(&dir).ok();
}
