use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fourfour_analysis::analyze::Control;
use fourfour_analysis::service::Pass;
use pioneer_library::{analysis_stage, LocalLibrary};
use pioneer_usb_writer::models;

use crate::analysis_backend::{resolve_runner, AnalyzerCache, BackendEnv, Runner, TrackEngine};
use crate::analysis_python::{analyze_one_track, resolve_python};

pub(crate) type SharedLibrary = Arc<Mutex<LocalLibrary>>;

pub(crate) type AnalysisWorkItem = (i64, PathBuf);
pub(crate) type SharedAnalysisQueue = Arc<Mutex<VecDeque<AnalysisWorkItem>>>;

/// What one background job computes: the fast pass (waveform and key, one decode, no model),
/// the grid pass (Beat This! and fit), or everything at once (Python, or staging off).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Fast,
    Grid,
    Whole,
}

impl Stage {
    fn pass(self) -> Pass {
        match self {
            Stage::Fast => Pass::Fast,
            Stage::Grid => Pass::Grid,
            Stage::Whole => Pass::All,
        }
    }
}

struct BackgroundJob {
    cancel: Arc<AtomicBool>,
    threads: usize,
}

#[derive(Default)]
struct Jobs {
    grid_queue: VecDeque<AnalysisWorkItem>,
    background: HashMap<i64, BackgroundJob>,
    now: HashMap<i64, Arc<AtomicBool>>,
    preempted: HashSet<i64>,
    sealed: bool,
}

pub(crate) struct BackgroundTask {
    pub(crate) track_id: i64,
    pub(crate) source_path: PathBuf,
    pub(crate) stage: Stage,
    pub(crate) threads: usize,
    pub(crate) cancel: Arc<AtomicBool>,
}

pub(crate) enum NowClaim {
    Granted { cancel: Arc<AtomicBool> },
    AlreadyRunning,
    NoRun,
}

/// Live analysis run state — exposes the work queue for reorder / enqueue while running.
#[derive(Clone)]
pub(crate) struct AnalysisCoordinator {
    active_queue: Arc<Mutex<Option<SharedAnalysisQueue>>>,
    total: Arc<AtomicU32>,
    completed: Arc<AtomicU32>,
    errors: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    background_paused: Arc<AtomicBool>,
    jobs: Arc<Mutex<Jobs>>,
    staged: bool,
    pub(crate) analyzers: AnalyzerCache,
}

impl Default for AnalysisCoordinator {
    fn default() -> Self {
        Self {
            active_queue: Arc::new(Mutex::new(None)),
            total: Arc::new(AtomicU32::new(0)),
            completed: Arc::new(AtomicU32::new(0)),
            errors: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            background_paused: Arc::new(AtomicBool::new(false)),
            jobs: Arc::new(Mutex::new(Jobs::default())),
            staged: true,
            analyzers: AnalyzerCache::default(),
        }
    }
}

impl AnalysisCoordinator {
    /// With staging off the Rust backend analyses each track in one piece, as before staging existed.
    pub(crate) fn with_staging(mut self, staged: bool) -> Self {
        self.staged = staged;
        self
    }

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
        self.completed.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        *self.active_queue.lock().expect("analysis coordinator lock") = Some(queue);
        self.total.store(total, Ordering::Relaxed);
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.with_queue(|queue| queue.clear());
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.grid_queue.clear();
        jobs.background.values().for_each(|job| job.cancel.store(true, Ordering::Relaxed));
        jobs.now.values().for_each(|cancel| cancel.store(true, Ordering::Relaxed));
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub(crate) fn end_run(&self) {
        *self.active_queue.lock().expect("analysis coordinator lock") = None;
        self.total.store(0, Ordering::Relaxed);
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.sealed = false;
        jobs.grid_queue.clear();
        drop(jobs);
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

    pub(crate) fn background_paused(&self) -> bool {
        self.background_paused.load(Ordering::Relaxed)
    }

    fn pause_flag(&self) -> Arc<AtomicBool> {
        self.background_paused.clone()
    }

    fn queue_len(&self) -> usize {
        let grid = self.jobs.lock().map_or(0, |jobs| jobs.grid_queue.len());
        self.with_queue(|queue| queue.len()).unwrap_or(0) + grid
    }

    fn push_grid(&self, item: AnalysisWorkItem) {
        self.jobs.lock().expect("analysis jobs lock").grid_queue.push_back(item);
    }

    /// Pops the next track and registers it as a background job in one step, so a priority request can
    /// never miss a track that is between a queue and a worker. Fast passes of the whole library come
    /// first; grid passes follow. `stages_of` tells which stages a track already has stored.
    pub(crate) fn take_next(
        &self,
        staged: bool,
        stages_of: impl Fn(i64) -> u8,
        threads_for: impl FnOnce(usize) -> usize,
    ) -> Option<BackgroundTask> {
        let mut guard = self.jobs.lock().ok()?;
        let jobs = &mut *guard;
        let fresh = self.with_queue(|queue| queue.pop_front()).flatten();
        let (track_id, source_path, stage) = match fresh {
            Some((id, path)) if !staged => (id, path, Stage::Whole),
            Some((id, path)) if stages_of(id) & FAST_STAGES == FAST_STAGES => (id, path, Stage::Grid),
            Some((id, path)) => (id, path, Stage::Fast),
            None => {
                let (id, path) = jobs.grid_queue.pop_front()?;
                (id, path, Stage::Grid)
            }
        };
        let waiting = self.with_queue(|queue| queue.len()).unwrap_or(0) + jobs.grid_queue.len();
        let threads = match stage {
            Stage::Fast => 1,
            Stage::Grid | Stage::Whole => threads_for(waiting + jobs.background.len() + 1),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.background.insert(track_id, BackgroundJob { cancel: cancel.clone(), threads });
        Some(BackgroundTask { track_id, source_path, stage, threads, cancel })
    }

    /// Ends a background job. True when a priority request preempted it (its result is to be dropped).
    pub(crate) fn finish_background(&self, track_id: i64) -> bool {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.background.remove(&track_id);
        jobs.preempted.remove(&track_id)
    }

    /// Moves a track onto the now lane: out of the queue, or preempting its narrow background job.
    /// `wide_threads` is the thread count of the now lane; a background job already that wide stays.
    pub(crate) fn claim_now(&self, track_id: i64, wide_threads: usize) -> NowClaim {
        let mut guard = self.jobs.lock().expect("analysis jobs lock");
        let jobs = &mut *guard;
        if !self.is_active() || jobs.sealed {
            return NowClaim::NoRun;
        }
        if jobs.now.contains_key(&track_id) {
            return NowClaim::AlreadyRunning;
        }
        match jobs.background.get(&track_id) {
            Some(job) if job.threads >= wide_threads => return NowClaim::AlreadyRunning,
            Some(job) => {
                job.cancel.store(true, Ordering::Relaxed);
                jobs.preempted.insert(track_id);
            }
            None => {
                let from_queue = self
                    .with_queue(|queue| queue.iter().position(|(id, _)| *id == track_id).and_then(|pos| queue.remove(pos)))
                    .flatten()
                    .is_some();
                let from_grid_queue = jobs
                    .grid_queue
                    .iter()
                    .position(|(id, _)| *id == track_id)
                    .and_then(|pos| jobs.grid_queue.remove(pos))
                    .is_some();
                let was_queued = from_queue || from_grid_queue;
                if !was_queued {
                    self.bump_total(1);
                }
            }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.now.insert(track_id, cancel.clone());
        self.background_paused.store(true, Ordering::Relaxed);
        NowClaim::Granted { cancel }
    }

    pub(crate) fn release_now(&self, track_id: i64) {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.now.remove(&track_id);
        self.background_paused.store(!jobs.now.is_empty(), Ordering::Relaxed);
    }

    /// Closes the run to new now-lane claims when none is running. The run loop ends only through this.
    fn seal_if_idle(&self) -> bool {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        if jobs.now.is_empty() {
            jobs.sealed = true;
        }
        jobs.sealed
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
    /// The waveform of `track_id` is stored; tempo, grid and key follow with the full result.
    fn partial(&self, track_id: i64);
    fn complete(&self);
}

pub(crate) enum RunOutcome {
    Merged,
    NothingToDo,
    Finished { errors: u32, total: u32 },
}

const FAST_STAGES: u8 = analysis_stage::WAVEFORM | analysis_stage::KEY;
const PYTHON_MAX_IN_FLIGHT: usize = 3;
const RUST_TRACK_TIMEOUT: Duration = Duration::from_secs(900);
const SCHEDULER_POLL: Duration = Duration::from_millis(100);

type TrackResult = Result<(models::AnalysisResult, String), String>;
type PartialStore = Arc<dyn Fn(models::AnalysisResult) + Send + Sync>;

struct JobControl {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    on_partial: Option<PartialStore>,
}

async fn analyze_one_track_rust(
    engine: Arc<dyn TrackEngine>,
    source_path: PathBuf,
    threads: usize,
    pass: Pass,
    job: JobControl,
) -> TrackResult {
    let started = Instant::now();
    let handle = tokio::task::spawn_blocking(move || {
        let control = Control { cancel: &job.cancel, pause: &job.pause, on_waveform: None };
        let on_partial = job.on_partial.as_deref().map(|f| f as &(dyn Fn(models::AnalysisResult) + Sync));
        engine.analyze(&source_path, &control, threads, pass, on_partial)
    });
    let joined = tokio::time::timeout(RUST_TRACK_TIMEOUT, handle)
        .await
        .map_err(|_| format!("TIMEOUT after {}s", RUST_TRACK_TIMEOUT.as_secs()))?;
    let result = joined.map_err(|e| format!("analysis task failed: {e}"))??;
    Ok((result, format!("{:.1}s rust {pass:?} x{threads}", started.elapsed().as_secs_f64())))
}

fn file_name_of(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string()
}

fn record_outcome(
    shared: &SharedLibrary,
    sink: &Arc<dyn AnalysisSink>,
    coordinator: &AnalysisCoordinator,
    track_id: i64,
    source_path: &Path,
    stage: Stage,
    outcome: TrackResult,
) -> Result<(), String> {
    let completed = coordinator.completed.fetch_add(1, Ordering::Relaxed) + 1;
    let total = coordinator.total().max(completed);
    let file_name = file_name_of(source_path);
    match outcome {
        Ok((mut result, detail)) => {
            let t_db = Instant::now();
            let lib = shared.lock().map_err(|e| e.to_string())?;
            let stored_track = lib.get_track(track_id).map_err(|e| e.to_string())?;
            if stage == Stage::Grid {
                if let Some(track) = &stored_track {
                    result.key = track.key.clone();
                }
            }
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
            if let Some(mut track) = stored_track {
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
            coordinator.errors.fetch_add(1, Ordering::Relaxed);
            sink.log(LogLevel::Err, format!("[{completed}/{total}] {file_name} — FAILED: {e}"));
            sink.progress(completed, total, format!("Failed {file_name} ({completed}/{total})"), track_id);
        }
    }
    Ok(())
}

fn store_fast_pass(
    shared: &SharedLibrary,
    sink: &Arc<dyn AnalysisSink>,
    track_id: i64,
    source_path: &Path,
    result: &models::AnalysisResult,
    detail: &str,
) -> Result<(), String> {
    let lib = shared.lock().map_err(|e| e.to_string())?;
    if !result.key.is_empty() {
        if let Some(mut track) = lib.get_track(track_id).map_err(|e| e.to_string())? {
            track.key = result.key.clone();
            lib.update_track(track_id, &track).map_err(|e| e.to_string())?;
        }
    }
    lib.set_analysis_stages(track_id, result, FAST_STAGES).map_err(|e| e.to_string())?;
    drop(lib);
    sink.log(LogLevel::Dim, format!("{} waveform+key stored key={} {detail}", file_name_of(source_path), result.key));
    sink.partial(track_id);
    Ok(())
}

fn partial_store(shared: &SharedLibrary, sink: &Arc<dyn AnalysisSink>, track_id: i64, source_path: &Path, started: Instant) -> PartialStore {
    let (shared, sink, file_name) = (shared.clone(), sink.clone(), file_name_of(source_path));
    Arc::new(move |partial| {
        let stored = shared
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|lib| lib.set_analysis_stages(track_id, &partial, analysis_stage::WAVEFORM).map_err(|e| e.to_string()));
        match stored {
            Ok(()) => {
                sink.log(LogLevel::Dim, format!("{file_name} waveform ready {:.1}s", started.elapsed().as_secs_f64()));
                sink.partial(track_id);
            }
            Err(e) => sink.log(LogLevel::Err, format!("{file_name} waveform not stored: {e}")),
        }
    })
}

struct NowGuard {
    coordinator: AnalysisCoordinator,
    track_id: i64,
}

impl Drop for NowGuard {
    fn drop(&mut self) {
        self.coordinator.release_now(self.track_id);
    }
}

/// The now lane: one track, every thread, background analysis paused, waveform stored first.
async fn run_now_lane(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    cancel: Arc<AtomicBool>,
    runner: Runner,
    wide_threads: usize,
    sink: Arc<dyn AnalysisSink>,
) -> Result<(), String> {
    let (track_id, source_path) = item;
    let _guard = NowGuard { coordinator: coordinator.clone(), track_id };
    let started = Instant::now();
    let on_partial = partial_store(&shared, &sink, track_id, &source_path, started);
    let outcome = match runner {
        Runner::Rust { engine } => {
            sink.log(LogLevel::Info, format!("Priority: {} on the now lane, threads={wide_threads}", file_name_of(&source_path)));
            let job = JobControl { cancel, pause: Arc::new(AtomicBool::new(false)), on_partial: Some(on_partial) };
            analyze_one_track_rust(engine, source_path.clone(), wide_threads, Pass::All, job).await
        }
        Runner::Python { command } => {
            sink.log(LogLevel::Info, format!("Priority: {} starts at once, background paused", file_name_of(&source_path)));
            let bands_path = source_path.clone();
            let bands = tokio::task::spawn_blocking(move || {
                if let Ok(waves) = crate::band_analysis::measure_waveforms(&bands_path) {
                    on_partial(fourfour_analysis::service::waveform_only_result(&waves));
                }
            });
            let result = analyze_one_track(&command, track_id, &source_path).await;
            bands.await.ok();
            result
        }
    };
    record_outcome(&shared, &sink, &coordinator, track_id, &source_path, Stage::Whole, outcome)
}

/// Analyse `item` as fast as possible: now lane inside an active run, a run of its own otherwise.
pub(crate) async fn prioritize_track(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
) -> Result<(), String> {
    let weights = fourfour_analysis::service::locate_models(env.app_data_dir.as_deref());
    let (runner, _) = resolve_runner(&env, weights, &coordinator.analyzers, resolve_python);
    prioritize_with_runner(shared, coordinator, item, sink, env, runner).await
}

async fn prioritize_with_runner(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
    runner: Runner,
) -> Result<(), String> {
    let wide_threads = match &runner {
        Runner::Rust { engine } => engine.plan(1).threads_per_track,
        Runner::Python { .. } => 0,
    };
    match coordinator.claim_now(item.0, wide_threads) {
        NowClaim::AlreadyRunning => Ok(()),
        NowClaim::NoRun => run_with_runner(shared, coordinator, vec![item], sink, env, Some(runner)).await.map(|_| ()),
        NowClaim::Granted { cancel } => run_now_lane(shared, coordinator, item, cancel, runner, wide_threads, sink).await,
    }
}

/// Run analysis for the given tracks with the configured backend. Used by bulk and per-selection commands.
pub(crate) async fn run_track_analysis(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
) -> Result<RunOutcome, String> {
    run_with_runner(shared, coordinator, pending, sink, env, None).await
}

async fn run_with_runner(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
    chosen_runner: Option<Runner>,
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

    let runner = match chosen_runner {
        Some(runner) => runner,
        None => {
            let weights = fourfour_analysis::service::locate_models(env.app_data_dir.as_deref());
            let (runner, notes) = resolve_runner(&env, weights, &coordinator.analyzers, resolve_python);
            for note in notes {
                sink.log(LogLevel::Info, note);
            }
            runner
        }
    };
    let max_in_flight = match &runner {
        Runner::Python { command } => {
            sink.log(LogLevel::Info, format!("Starting analysis: {initial_total} tracks, python={command}"));
            PYTHON_MAX_IN_FLIGHT
        }
        Runner::Rust { engine } => {
            let plan = engine.plan(usize::MAX);
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

    let mut join_set: tokio::task::JoinSet<(i64, PathBuf, Stage, TrackResult)> = tokio::task::JoinSet::new();

    let staged = coordinator.staged && matches!(runner, Runner::Rust { .. });
    loop {
        while join_set.len() < max_in_flight && !coordinator.is_cancelled() && !coordinator.background_paused() {
            let stored_stages = |id: i64| shared.lock().ok().and_then(|lib| lib.analysis_stages(id).ok().flatten()).unwrap_or(0);
            let task = coordinator.take_next(staged, stored_stages, |jobs_left| match &runner {
                Runner::Python { .. } => 0,
                Runner::Rust { engine } => engine.plan(jobs_left).threads_per_track,
            });
            let Some(task) = task else {
                break;
            };
            match runner.clone() {
                Runner::Python { command } => {
                    join_set.spawn(async move {
                        let result = analyze_one_track(&command, task.track_id, &task.source_path).await;
                        (task.track_id, task.source_path, task.stage, result)
                    });
                }
                Runner::Rust { engine } => {
                    let on_partial = (task.stage == Stage::Fast)
                        .then(|| partial_store(&shared, &sink, task.track_id, &task.source_path, Instant::now()));
                    let job = JobControl { cancel: task.cancel, pause: coordinator.pause_flag(), on_partial };
                    join_set.spawn(async move {
                        let result = analyze_one_track_rust(engine, task.source_path.clone(), task.threads, task.stage.pass(), job).await;
                        (task.track_id, task.source_path, task.stage, result)
                    });
                }
            }
        }

        if join_set.is_empty() {
            if coordinator.queue_len() == 0 && coordinator.seal_if_idle() {
                break;
            }
            tokio::time::sleep(SCHEDULER_POLL).await;
            continue;
        }

        let Ok(task_result) = tokio::time::timeout(SCHEDULER_POLL, join_set.join_next()).await else {
            continue;
        };
        let (track_id, source_path, stage, analysis_result) = task_result
            .ok_or_else(|| "analysis task ended".to_string())?
            .map_err(|e| format!("Task panicked: {e}"))?;

        if coordinator.finish_background(track_id) {
            continue;
        }
        match (stage, analysis_result) {
            (Stage::Fast, Ok((result, detail))) => {
                store_fast_pass(&shared, &sink, track_id, &source_path, &result, &detail)?;
                coordinator.push_grid((track_id, source_path));
            }
            (_, outcome) => record_outcome(&shared, &sink, &coordinator, track_id, &source_path, stage, outcome)?,
        }
    }

    let completed = coordinator.completed.load(Ordering::Relaxed);
    let errors = coordinator.errors.load(Ordering::Relaxed);
    let total = coordinator.total();
    sink.log(LogLevel::Info, format!("Analysis complete: {completed} done, {errors} errors out of {total}"));
    sink.complete();
    Ok(RunOutcome::Finished { errors, total })
}

#[cfg(test)]
#[path = "analysis_run_tests.rs"]
mod tests;
