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

#[allow(dead_code)]
#[path = "../analysis_order.rs"]
mod analysis_order;
#[allow(dead_code)]
#[path = "../analysis_run.rs"]
mod analysis_run;

#[allow(dead_code)]
#[path = "../analysis_payload.rs"]
mod analysis_payload;
#[allow(dead_code)]
#[path = "../dto.rs"]
mod dto;
#[allow(dead_code)]
#[path = "../track_infos.rs"]
mod track_infos;

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

fn process_cpu_s() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat.rsplit_once(") ").map_or(Vec::new(), |(_, rest)| rest.split_whitespace().collect());
    let ticks = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
    (ticks(11) + ticks(12)) / 100.0
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
    let mut fast_first = false;
    let mut profile = 0usize;
    let mut store_detail = false;
    let mut dump: Option<PathBuf> = None;
    let mut ui_busy: Option<(Duration, Duration)> = None;
    let mut probe_reads: Option<Duration> = None;
    let mut ui_busy_file: Option<PathBuf> = None;
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
            "--fast-first" => fast_first = true,
            "--profile" => profile = it.next().unwrap().parse().unwrap(),
            "--dump" => dump = Some(PathBuf::from(it.next().unwrap())),
            "--store-detail" => store_detail = true,
            "--ui-busy" => {
                let spec = it.next().unwrap();
                let (busy, period) = spec.split_once('/').expect("--ui-busy <busy ms>/<period ms>");
                ui_busy = Some((Duration::from_millis(busy.parse().unwrap()), Duration::from_millis(period.parse().unwrap())));
            }
            "--ui-busy-file" => ui_busy_file = Some(PathBuf::from(it.next().unwrap())),
            "--probe-reads" => probe_reads = Some(Duration::from_millis(it.next().unwrap().parse().unwrap())),
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

    let app_data_dir = store_detail.then(|| dir.clone());
    let env = BackendEnv { override_backend: Some(backend), max_threads: threads, app_data_dir, ..BackendEnv::default() };
    let coordinator = AnalysisCoordinator::default().with_staging(!unstaged);
    let coordinator = if fast_first { coordinator.with_whole_jobs(0) } else { coordinator };
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

    let ui = ui_busy.map(|(busy, period)| {
        let (stop, coordinator) = (stop.clone(), coordinator.clone());
        std::thread::spawn(move || simulate_ui(&stop, &coordinator, started, busy, period))
    });
    let ui_from_file = ui_busy_file.map(|file| {
        let (stop, coordinator) = (stop.clone(), coordinator.clone());
        std::thread::spawn(move || follow_ui_busy_file(&stop, &coordinator, &file))
    });
    let reads = probe_reads.map(|every| {
        let (stop, shared, ids) = (stop.clone(), shared.clone(), ids.clone());
        std::thread::spawn(move || probe_row_reads(&stop, &shared, &ids, every))
    });

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
    if let Some(ui) = ui {
        let [(free_cpu, free_s), (busy_cpu, busy_s)] = ui.join().unwrap();
        let (pauses, held) = coordinator.ui_pause_totals();
        println!(
            "UI busy={:?} held={pauses}x {:.1}s ({:.0}% of the run) cores_used_while_busy={:.1} cores_used_otherwise={:.1}",
            ui_busy.unwrap(),
            held.as_secs_f64(),
            held.as_secs_f64() / wall_s * 100.0,
            busy_cpu / busy_s,
            free_cpu / free_s
        );
    }
    if let Some(ui) = ui_from_file {
        let signals = ui.join().unwrap();
        let (pauses, held) = coordinator.ui_pause_totals();
        println!("UI file signals={signals} held={pauses}x {:.1}s ({:.0}% of the run)", held.as_secs_f64(), held.as_secs_f64() / wall_s * 100.0);
    }
    if let Some(reads) = reads {
        reads.join().unwrap().print();
    }

    let (errors, total) = match outcome {
        RunOutcome::Finished { errors, total } => (errors, total),
        _ => (0, 0),
    };
    let done = total - errors;
    let first = sink.first_result_ms.lock().unwrap().unwrap_or(0.0);
    println!(
        "BENCH backend={} tracks={} errors={errors} wall={wall_s:.1}s tracks_per_min={:.2} first_result={:.1}s peak_rss_tree={:.0}MB cpu={:.1}s",
        backend.name(),
        done,
        done as f64 / wall_s * 60.0,
        first / 1000.0,
        *peak.lock().unwrap(),
        process_cpu_s()
    );
    println!(
        "PERF {}",
        serde_json::json!({ "bench": "analysis_throughput", "values": {
            "backend": backend.name(), "staged": !unstaged, "tracks": done, "errors": errors,
            "wall_s": wall_s, "tracks_per_min": done as f64 / wall_s * 60.0,
            "first_result_s": first / 1000.0, "peak_rss_tree_mb": *peak.lock().unwrap(),
        }})
    );
    if let Some(analyzer) = coordinator.analyzers.loaded() {
        let status = analyzer.beat_status();
        println!(
            "BEAT runtime={} chunks_accelerated={} chunks_rten={} fallback={}",
            status.runtime,
            status.chunks_accelerated,
            status.chunks_rten,
            status.fallback.as_deref().unwrap_or("none")
        );
    }
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
    if store_detail {
        check_stored_detail(&dir.join("waveform-detail"), &paths);
    }
    std::fs::remove_dir_all(&dir).ok();
}

fn check_stored_detail(cache: &std::path::Path, paths: &[PathBuf]) {
    let stored = std::fs::read_dir(cache).map_or(0, |entries| {
        entries.flatten().filter(|e| e.path().extension().is_some_and(|ext| ext == "ffwd")).count()
    });
    let mut identical = 0;
    for path in paths {
        let t = Instant::now();
        let cached = waveform_detail::detail_bytes(cache, path).expect("detail from the cache");
        let cached_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (t, cpu) = (Instant::now(), process_cpu_s());
        let measured = band_analysis::measure_detail(path).expect("measure detail").to_bytes();
        let measured_ms = t.elapsed().as_secs_f64() * 1000.0;
        let measured_cpu_s = process_cpu_s() - cpu;
        let same = cached == measured;
        identical += same as usize;
        println!("DETAIL identical={same} bytes={} cached_ms={cached_ms:.1} measured_ms={measured_ms:.0} measured_cpu={measured_cpu_s:.2}s {}", cached.len(), path.display());
    }
    println!("DETAIL stored={stored}/{} identical={identical}/{}", paths.len(), paths.len());
}

fn simulate_ui(stop: &AtomicBool, coordinator: &AnalysisCoordinator, started: Instant, busy: Duration, period: Duration) -> [(f64, f64); 2] {
    const SIGNAL_EVERY: Duration = Duration::from_millis(200);
    let mut cpu_and_seconds = [(0.0, 0.0); 2];
    let (mut last_cpu, mut last_at) = (process_cpu_s(), Instant::now());
    while !stop.load(Ordering::Relaxed) {
        let phase = Duration::from_nanos((started.elapsed().as_nanos() % period.as_nanos()) as u64);
        let in_busy = phase < busy;
        if in_busy {
            coordinator.ui_busy();
            std::thread::sleep(SIGNAL_EVERY.min(busy - phase));
        } else {
            std::thread::sleep((period - phase).min(Duration::from_millis(50)));
        }
        let (cpu, at) = (process_cpu_s(), Instant::now());
        let slot = &mut cpu_and_seconds[in_busy as usize];
        slot.0 += cpu - last_cpu;
        slot.1 += (at - last_at).as_secs_f64();
        (last_cpu, last_at) = (cpu, at);
    }
    cpu_and_seconds
}

struct ReadProbe {
    waits_ms: Vec<f64>,
    calls_ms: Vec<f64>,
}

impl ReadProbe {
    fn print(self) {
        let pct = |v: &[f64], p: f64| {
            let mut v = v.to_vec();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.get(((v.len() as f64 * p) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(f64::NAN)
        };
        let over = |limit: f64| self.waits_ms.iter().filter(|&&w| w > limit).count();
        println!(
            "LOCK reads={} wait_p50={:.2}ms wait_p95={:.2}ms wait_p99={:.2}ms wait_max={:.1}ms waits_over_4ms={} waits_over_16ms={} call_p50={:.2}ms call_p95={:.2}ms",
            self.waits_ms.len(),
            pct(&self.waits_ms, 0.5),
            pct(&self.waits_ms, 0.95),
            pct(&self.waits_ms, 0.99),
            self.waits_ms.iter().copied().fold(0.0, f64::max),
            over(4.0),
            over(16.0),
            pct(&self.calls_ms, 0.5),
            pct(&self.calls_ms, 0.95),
        );
    }
}

fn probe_row_reads(stop: &AtomicBool, shared: &Mutex<LocalLibrary>, ids: &[i64], every: Duration) -> ReadProbe {
    let mut probe = ReadProbe { waits_ms: Vec::new(), calls_ms: Vec::new() };
    let mut n = 0usize;
    while !stop.load(Ordering::Relaxed) {
        let id = ids[n % ids.len()];
        n += 1;
        let t = Instant::now();
        let lib = shared.lock().unwrap();
        probe.waits_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        analysis_payload::build_analysis_json(&lib, id).ok();
        drop(lib);
        probe.calls_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        std::thread::sleep(every);
    }
    probe
}

fn follow_ui_busy_file(stop: &AtomicBool, coordinator: &AnalysisCoordinator, file: &std::path::Path) -> u64 {
    let len = || std::fs::metadata(file).map_or(0, |m| m.len());
    let mut seen = len();
    let mut signals = 0;
    while !stop.load(Ordering::Relaxed) {
        let now = len();
        if now > seen {
            coordinator.ui_busy();
            signals += now - seen;
        }
        seen = now;
        std::thread::sleep(Duration::from_millis(20));
    }
    signals
}
