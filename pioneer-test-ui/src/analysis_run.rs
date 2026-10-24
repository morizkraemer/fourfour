use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;

use crate::analysis_backend::{resolve_runner, AnalyzerCache, BackendEnv, Runner};
use crate::analysis_python::{analyze_one_track, resolve_python};

pub(crate) type SharedLibrary = Arc<Mutex<LocalLibrary>>;

pub(crate) type AnalysisWorkItem = (i64, PathBuf);
pub(crate) type SharedAnalysisQueue = Arc<Mutex<VecDeque<AnalysisWorkItem>>>;

/// Live analysis run state — exposes the work queue for reorder / enqueue while running.
#[derive(Clone)]
pub(crate) struct AnalysisCoordinator {
    active_queue: Arc<Mutex<Option<SharedAnalysisQueue>>>,
    total: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    pub(crate) analyzers: AnalyzerCache,
}

impl Default for AnalysisCoordinator {
    fn default() -> Self {
        Self {
            active_queue: Arc::new(Mutex::new(None)),
            total: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            analyzers: AnalyzerCache::default(),
        }
    }
}

impl AnalysisCoordinator {
    pub(crate) fn is_active(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Returns true when this caller owns the run (false = work was merged into an active run).
    pub(crate) fn try_begin(&self) -> bool {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    pub(crate) fn register_run(&self, queue: SharedAnalysisQueue, total: u32) {
        self.cancel.store(false, Ordering::Relaxed);
        *self.active_queue.lock().expect("analysis coordinator lock") = Some(queue);
        self.total.store(total, Ordering::Relaxed);
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.with_queue(|queue| queue.clear());
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub(crate) fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub(crate) fn end_run(&self) {
        *self.active_queue.lock().expect("analysis coordinator lock") = None;
        self.total.store(0, Ordering::Relaxed);
        self.running.store(false, Ordering::Release);
    }

    pub(crate) fn with_queue<R>(&self, f: impl FnOnce(&mut VecDeque<AnalysisWorkItem>) -> R) -> Option<R> {
        let guard = self.active_queue.lock().ok()?;
        let q = guard.as_ref()?;
        let mut deque = q.lock().ok()?;
        Some(f(&mut deque))
    }

    pub(crate) fn bump_total(&self, delta: u32) {
        if delta > 0 {
            self.total.fetch_add(delta, Ordering::Relaxed);
        }
    }

    pub(crate) fn total(&self) -> u32 {
        self.total.load(Ordering::Relaxed)
    }
}

pub(crate) fn resolve_analysis_work(
    lib: &LocalLibrary,
    ids: &[i64],
) -> Result<Vec<AnalysisWorkItem>, String> {
    let mut work = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(track) = lib.get_track(id).map_err(|e| e.to_string())? {
            work.push((id, track.source_path));
        }
    }
    Ok(work)
}

pub(crate) fn merge_into_analysis_queue(
    coordinator: &AnalysisCoordinator,
    items: Vec<AnalysisWorkItem>,
    front: bool,
) -> u32 {
    let mut added = 0u32;
    coordinator.with_queue(|queue| {
        for item in items {
            let track_id = item.0;
            if let Some(pos) = queue.iter().position(|(id, _)| *id == track_id) {
                if front && pos > 0 {
                    let existing = queue.remove(pos).unwrap();
                    queue.push_front(existing);
                }
                continue;
            }
            if front {
                queue.push_front(item);
            } else {
                queue.push_back(item);
            }
            added += 1;
        }
    });
    if added > 0 {
        coordinator.bump_total(added);
    }
    added
}

#[derive(Clone, Copy)]
pub(crate) enum LogLevel {
    Info,
    Ok,
    Err,
    Dim,
}

impl LogLevel {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Ok => "ok",
            LogLevel::Err => "err",
            LogLevel::Dim => "dim",
        }
    }
}

pub(crate) trait AnalysisSink: Send + Sync + 'static {
    fn log(&self, level: LogLevel, message: String);
    fn progress(&self, current: u32, total: u32, message: String, track_id: i64);
    fn complete(&self);
}

pub(crate) enum RunOutcome {
    Merged,
    NothingToDo,
    Finished { errors: u32, total: u32 },
}

const PYTHON_MAX_IN_FLIGHT: usize = 3;
const TRACK_TIMEOUT: Duration = Duration::from_secs(120);

type TrackResult = Result<(models::AnalysisResult, String), String>;

async fn analyze_one_track_rust(
    analyzer: Arc<fourfour_analysis::service::Analyzer>,
    source_path: PathBuf,
    threads: usize,
    cancel: Arc<AtomicBool>,
) -> TrackResult {
    let started = Instant::now();
    let handle = tokio::task::spawn_blocking(move || {
        analyzer.analyze_with(&source_path, &cancel, threads).map_err(|e| e.to_string())
    });
    let joined = tokio::time::timeout(TRACK_TIMEOUT, handle)
        .await
        .map_err(|_| format!("TIMEOUT after {}s", TRACK_TIMEOUT.as_secs()))?;
    let result = joined.map_err(|e| format!("analysis task failed: {e}"))??;
    Ok((result, format!("{:.1}s rust x{threads}", started.elapsed().as_secs_f64())))
}

/// Run analysis for the given tracks with the configured backend. Used by bulk and per-selection commands.
pub(crate) async fn run_track_analysis(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
) -> Result<RunOutcome, String> {
    if pending.is_empty() {
        sink.log(LogLevel::Info, "No unanalyzed tracks — nothing to do".to_string());
        return Ok(RunOutcome::NothingToDo);
    }

    if !coordinator.try_begin() {
        merge_into_analysis_queue(&coordinator, pending, false);
        return Ok(RunOutcome::Merged);
    }

    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from(pending)));
    let initial_total = queue.lock().map_err(|e| e.to_string())?.len() as u32;
    coordinator.register_run(queue.clone(), initial_total);
    struct RunGuard {
        coordinator: AnalysisCoordinator,
    }
    impl Drop for RunGuard {
        fn drop(&mut self) {
            self.coordinator.end_run();
        }
    }
    let _run_guard = RunGuard { coordinator: coordinator.clone() };

    let weights = fourfour_analysis::service::locate_models(env.app_data_dir.as_deref());
    let (runner, notes) = resolve_runner(&env, weights, &coordinator.analyzers, resolve_python);
    for note in notes {
        sink.log(LogLevel::Info, note);
    }
    let max_in_flight = match &runner {
        Runner::Python { command } => {
            sink.log(LogLevel::Info, format!("Starting analysis: {initial_total} tracks, python={command}"));
            PYTHON_MAX_IN_FLIGHT
        }
        Runner::Rust { analyzer } => {
            let plan = analyzer.plan(usize::MAX);
            sink.log(
                LogLevel::Info,
                format!(
                    "Starting analysis: {initial_total} tracks, rust workers={} threads/track={}",
                    plan.workers, plan.threads_per_track
                ),
            );
            plan.workers
        }
    };

    let mut join_set: tokio::task::JoinSet<(i64, PathBuf, TrackResult)> = tokio::task::JoinSet::new();
    let mut completed: u32 = 0;
    let mut errors: u32 = 0;

    loop {
        while join_set.len() < max_in_flight && !coordinator.is_cancelled() {
            let (next, queued) = {
                let mut q = queue.lock().map_err(|e| e.to_string())?;
                (q.pop_front(), q.len())
            };
            let Some((track_id, source_path)) = next else {
                break;
            };
            match runner.clone() {
                Runner::Python { command } => {
                    join_set.spawn(async move {
                        let result = analyze_one_track(&command, track_id, &source_path).await;
                        (track_id, source_path, result)
                    });
                }
                Runner::Rust { analyzer } => {
                    let threads = analyzer.plan(queued + join_set.len() + 1).threads_per_track;
                    let run_cancel = coordinator.cancel_flag();
                    join_set.spawn(async move {
                        let result = analyze_one_track_rust(analyzer, source_path.clone(), threads, run_cancel).await;
                        (track_id, source_path, result)
                    });
                }
            }
        }

        if join_set.is_empty() {
            break;
        }

        let task_result = join_set
            .join_next()
            .await
            .ok_or_else(|| "analysis task ended".to_string())?;
        let (track_id, source_path, analysis_result) =
            task_result.map_err(|e| format!("Task panicked: {e}"))?;

        completed += 1;
        let total = coordinator.total().max(completed);
        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        match analysis_result {
            Ok((result, detail)) => {
                sink.log(
                    LogLevel::Ok,
                    format!(
                        "[{completed}/{total}] {file_name} — bpm={:.1} key={} beats={} cues={} {detail}",
                        result.bpm,
                        result.key,
                        result.beat_grid.beats.len(),
                        result.cue_points.len(),
                    ),
                );
                let t_db = Instant::now();
                let lib = shared.lock().map_err(|e| e.to_string())?;
                if let Some(mut track) = lib.get_track(track_id).map_err(|e| e.to_string())? {
                    track.tempo = (result.bpm * 100.0) as u32;
                    track.key = result.key.clone();
                    lib.update_track(track_id, &track).map_err(|e| e.to_string())?;
                }
                lib.set_analysis(track_id, &result).map_err(|e| e.to_string())?;
                drop(lib);
                sink.log(
                    LogLevel::Dim,
                    format!("[{completed}/{total}] {file_name} DB+ANLZ write {:.1}s", t_db.elapsed().as_secs_f64()),
                );
                sink.progress(completed, total, format!("Analyzed {file_name} ({completed}/{total})"), track_id);
            }
            Err(e) => {
                errors += 1;
                sink.log(LogLevel::Err, format!("[{completed}/{total}] {file_name} — FAILED: {e}"));
                sink.progress(completed, total, format!("Failed {file_name} ({completed}/{total})"), track_id);
            }
        }
    }

    let total = coordinator.total();
    sink.log(LogLevel::Info, format!("Analysis complete: {completed} done, {errors} errors out of {total}"));
    sink.complete();
    Ok(RunOutcome::Finished { errors, total })
}
