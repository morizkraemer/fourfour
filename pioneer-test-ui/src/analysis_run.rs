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
use crate::analysis_python::{analyze_one_track, resolve_python_checked};

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
    stage: Stage,
}

/// How many jobs that run the beat model may be in flight: `max` in all, `whole` while new tracks still wait
/// for their fast pass (each of those is analysed in one piece, so the model starts on the first track).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelJobs {
    pub(crate) max: usize,
    pub(crate) whole: usize,
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

#[derive(Default)]
struct PauseReasons {
    now_lane: bool,
    ui_busy_since: Option<Instant>,
    ui_busy_until: Option<Instant>,
    ui_pauses: u32,
    ui_paused: Duration,
}

pub(crate) const UI_IDLE_BEFORE_RESUME: Duration = Duration::from_millis(500);

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
    pause_reasons: Arc<Mutex<PauseReasons>>,
    jobs: Arc<Mutex<Jobs>>,
    staged: bool,
    whole_jobs: Option<usize>,
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
            pause_reasons: Arc::new(Mutex::new(PauseReasons::default())),
            jobs: Arc::new(Mutex::new(Jobs::default())),
            staged: true,
            whole_jobs: None,
            analyzers: AnalyzerCache::default(),
        }
    }
}

impl AnalysisCoordinator {
    /// With staging off the Rust backend analyses each track in one piece, as before staging existed.
    #[allow(dead_code)]
    pub(crate) fn with_staging(mut self, staged: bool) -> Self {
        self.staged = staged;
        self
    }

    /// Model jobs that start on new tracks while fast passes are still to come; 0 runs every fast pass first.
    #[allow(dead_code)]
    pub(crate) fn with_whole_jobs(mut self, jobs: usize) -> Self {
        self.whole_jobs = Some(jobs);
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
        let mut reasons = self.pause_reasons.lock().expect("analysis pause lock");
        reasons.ui_pauses = 0;
        reasons.ui_paused = Duration::ZERO;
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

    fn set_paused_by_now_lane(&self, now_lane: bool) {
        let mut reasons = self.pause_reasons.lock().expect("analysis pause lock");
        reasons.now_lane = now_lane;
        self.background_paused.store(now_lane || reasons.ui_busy_until.is_some(), Ordering::Relaxed);
    }

    pub(crate) fn ui_busy(&self) {
        if !self.is_active() {
            return;
        }
        let now = Instant::now();
        let mut reasons = self.pause_reasons.lock().expect("analysis pause lock");
        let idle_timer_running = reasons.ui_busy_until.is_some();
        reasons.ui_busy_until = Some(now + UI_IDLE_BEFORE_RESUME);
        reasons.ui_busy_since.get_or_insert(now);
        self.background_paused.store(true, Ordering::Relaxed);
        drop(reasons);
        if !idle_timer_running {
            let coordinator = self.clone();
            std::thread::Builder::new()
                .name("analysis-ui-idle".into())
                .spawn(move || coordinator.resume_when_ui_idle())
                .ok();
        }
    }

    fn resume_when_ui_idle(&self) {
        loop {
            let wait = {
                let mut reasons = self.pause_reasons.lock().expect("analysis pause lock");
                let now = Instant::now();
                match reasons.ui_busy_until {
                    Some(until) if until > now => until - now,
                    _ => {
                        if let Some(since) = reasons.ui_busy_since.take() {
                            reasons.ui_pauses += 1;
                            reasons.ui_paused += now - since;
                        }
                        reasons.ui_busy_until = None;
                        self.background_paused.store(reasons.now_lane, Ordering::Relaxed);
                        return;
                    }
                }
            };
            std::thread::sleep(wait);
        }
    }

    pub(crate) fn ui_pause_totals(&self) -> (u32, Duration) {
        let reasons = self.pause_reasons.lock().expect("analysis pause lock");
        (reasons.ui_pauses, reasons.ui_paused)
    }

    fn pause_flag(&self) -> Arc<AtomicBool> {
        self.background_paused.clone()
    }

    fn queue_len(&self) -> usize {
        let grid = self.jobs.lock().map_or(0, |jobs| jobs.grid_queue.len());
        self.with_queue(|queue| queue.len()).unwrap_or(0) + grid
    }

    fn is_scheduled(&self, track_id: i64) -> bool {
        let jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.background.contains_key(&track_id)
            || jobs.now.contains_key(&track_id)
            || jobs.grid_queue.iter().any(|(id, _)| *id == track_id)
    }

    fn is_preempted(&self, track_id: i64) -> bool {
        self.jobs.lock().expect("analysis jobs lock").preempted.contains(&track_id)
    }

    /// Ends a fast pass whose result is stored and queues the track's grid pass in one step, so the track is
    /// never between the two where a merge of the same track would queue it again. A cancelled run queues nothing.
    fn hand_to_grid(&self, item: AnalysisWorkItem) {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.background.remove(&item.0);
        if jobs.preempted.remove(&item.0) || self.is_cancelled() {
            return;
        }
        jobs.grid_queue.push_back(item);
    }

    /// Pops the next track and registers it as a background job in one step, so a priority request can
    /// never miss a track that is between a queue and a worker. Fast passes of the whole library come
    /// first and grid passes follow, but the model never waits for them: while fewer than `whole_jobs`
    /// model jobs run, the next new track is analysed whole (one decode, waveform stored first).
    /// `stages_of` tells which stages a track already has stored. At most `limits.max` jobs that run the
    /// beat model are in flight: the model's threads are shared by all of them, so more would only hold
    /// more decoded audio in memory.
    pub(crate) fn take_next(
        &self,
        staged: bool,
        stages_of: impl Fn(i64) -> u8,
        limits: ModelJobs,
        threads_for: impl FnOnce(usize) -> usize,
    ) -> Option<BackgroundTask> {
        let mut guard = self.jobs.lock().ok()?;
        let jobs = &mut *guard;
        let model_running = jobs.background.values().filter(|job| job.stage != Stage::Fast).count();
        let model_room = model_running < limits.max;
        let fresh = loop {
            let Some((id, path)) = self.with_queue(|queue| queue.pop_front()).flatten() else {
                break None;
            };
            let stage = match () {
                () if !staged => Stage::Whole,
                () if stages_of(id) & FAST_STAGES == FAST_STAGES => Stage::Grid,
                () if model_running < limits.whole.min(limits.max) => Stage::Whole,
                () => Stage::Fast,
            };
            match stage {
                Stage::Grid if !model_room => jobs.grid_queue.push_back((id, path)),
                Stage::Whole if !model_room => {
                    self.with_queue(|queue| queue.push_front((id, path)));
                    return None;
                }
                _ => break Some((id, path, stage)),
            }
        };
        let (track_id, source_path, stage) = match fresh {
            Some(next) => next,
            None if model_room => {
                let (id, path) = jobs.grid_queue.pop_front()?;
                (id, path, Stage::Grid)
            }
            None => return None,
        };
        let waiting = self.with_queue(|queue| queue.len()).unwrap_or(0) + jobs.grid_queue.len();
        let threads = match stage {
            Stage::Fast => 1,
            Stage::Grid | Stage::Whole => threads_for(waiting + jobs.background.len() + 1),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.background.insert(track_id, BackgroundJob { cancel: cancel.clone(), threads, stage });
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
        self.set_paused_by_now_lane(true);
        NowClaim::Granted { cancel }
    }

    pub(crate) fn release_now(&self, track_id: i64) {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        jobs.now.remove(&track_id);
        self.set_paused_by_now_lane(!jobs.now.is_empty());
    }

    /// Closes the run to new work when nothing is running or waiting; the check and the closing are one step
    /// under the jobs lock, which every merge holds too. The run loop ends only through this.
    fn seal_if_idle(&self) -> bool {
        let mut jobs = self.jobs.lock().expect("analysis jobs lock");
        let queue_empty = self.with_queue(|queue| queue.is_empty()).unwrap_or(true);
        if jobs.now.is_empty() && jobs.grid_queue.is_empty() && queue_empty {
            jobs.sealed = true;
        }
        jobs.sealed
    }

    /// Adds tracks to the active run and returns how many were new, or `None` when no run can take them: it is
    /// closing (sealed) or has ended. Tracks that are queued, running or between two passes are left alone.
    pub(crate) fn merge_work(&self, items: &[AnalysisWorkItem], front: bool) -> Option<u32> {
        let jobs = self.jobs.lock().expect("analysis jobs lock");
        if jobs.sealed {
            return None;
        }
        let added = self.with_queue(|queue| {
            let mut added = 0u32;
            for item in items {
                let track_id = item.0;
                let scheduled = jobs.background.contains_key(&track_id)
                    || jobs.now.contains_key(&track_id)
                    || jobs.grid_queue.iter().any(|(id, _)| *id == track_id);
                if scheduled {
                    continue;
                }
                if let Some(pos) = queue.iter().position(|(id, _)| *id == track_id) {
                    if front && pos > 0 {
                        let existing = queue.remove(pos).unwrap();
                        queue.push_front(existing);
                    }
                    continue;
                }
                if front {
                    queue.push_front(item.clone());
                } else {
                    queue.push_back(item.clone());
                }
                added += 1;
            }
            added
        })?;
        self.bump_total(added);
        Some(added)
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

/// Work for an explicit (re-)analysis: the tracks' stored stages are reset first, so they run
/// the same staged passes as a first analysis and are pending until each one is stored.
pub(crate) fn rerun_work(lib: &LocalLibrary, ids: &[i64]) -> Result<Vec<AnalysisWorkItem>, String> {
    for &id in ids {
        lib.reset_analysis_stages(id).map_err(|e| e.to_string())?;
    }
    resolve_analysis_work(lib, ids)
}

/// Tracks to analyse at launch: partial analyses first (they resume at the missing stage), then,
/// when `include_new` is set, tracks that were never analysed.
pub(crate) fn resume_work(lib: &LocalLibrary, include_new: bool) -> Result<Vec<AnalysisWorkItem>, String> {
    let mut ids = lib.get_incomplete_analysis_ids().map_err(|e| e.to_string())?;
    if include_new {
        let partial: HashSet<i64> = ids.iter().copied().collect();
        let unanalyzed = lib.get_unanalyzed_track_ids().map_err(|e| e.to_string())?;
        ids.extend(unanalyzed.into_iter().filter(|id| !partial.contains(id)));
    }
    resolve_analysis_work(lib, &ids)
}

pub(crate) fn merge_into_analysis_queue(
    coordinator: &AnalysisCoordinator,
    items: Vec<AnalysisWorkItem>,
    front: bool,
) -> u32 {
    coordinator.merge_work(&items, front).unwrap_or(0)
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
/// The waveform detail cache under the app data dir (`get_waveform_detail` in main.rs reads it).
const DETAIL_CACHE_DIR: &str = "waveform-detail";
/// Model jobs in flight: one per this many model threads, so the next tracks decode while the model runs.
const MODEL_THREADS_PER_JOB: usize = 3;
const MIN_MODEL_JOBS: usize = 3;
/// Model jobs while fast passes are still to come; the other workers keep taking fast passes.
const WHOLE_JOBS_DURING_FAST_PASSES: usize = 2;
const RUST_TRACK_TIMEOUT: Duration = Duration::from_secs(900);
const SCHEDULER_POLL: Duration = Duration::from_millis(100);

type TrackResult = Result<(models::AnalysisResult, String), String>;
type PartialStore = Arc<dyn Fn(models::AnalysisResult) + Send + Sync>;

struct JobControl {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    on_partial: Option<PartialStore>,
    detail_cache: Option<PathBuf>,
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
        crate::analysis_priority::run_below_ui(|| {
            let control = Control { cancel: &job.cancel, pause: &job.pause, on_waveform: None };
            let detail_cache = job.detail_cache.as_deref().filter(|_| pass != Pass::Fast);
            std::thread::scope(|scope| {
                let (start_detail, detail_started) = std::sync::mpsc::channel::<()>();
                if let Some(dir) = detail_cache {
                    let (cancel, path) = (&job.cancel, &source_path);
                    scope.spawn(move || {
                        if detail_started.recv().is_ok() && !cancel.load(Ordering::Relaxed) {
                            if let Err(e) = crate::waveform_detail::warm_detail(dir, path) {
                                eprintln!("waveform detail of {} not cached: {e}", path.display());
                            }
                        }
                    });
                }
                let Some(store_partial) = job.on_partial.as_deref() else {
                    start_detail.send(()).ok();
                    return engine.analyze(&source_path, &control, threads, pass, None);
                };
                let store_then_start_detail = move |partial: models::AnalysisResult| {
                    store_partial(partial);
                    start_detail.send(()).ok();
                };
                engine.analyze(&source_path, &control, threads, pass, Some(&store_then_start_detail))
            })
        })?
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
    let file_name = file_name_of(source_path);
    if outcome.is_err() && coordinator.is_cancelled() {
        sink.log(LogLevel::Dim, format!("{file_name} stopped by cancel"));
        return Ok(());
    }
    let completed = coordinator.completed.fetch_add(1, Ordering::Relaxed) + 1;
    let total = coordinator.total().max(completed);
    let t_db = Instant::now();
    let grid = stage == Stage::Grid;
    let stored = outcome.and_then(|(result, detail)| {
        store_analysis(shared, track_id, source_path, &result, analysis_stage::ALL, |track, analysis| {
            if grid {
                if let Some(track) = track {
                    analysis.key = track.key.clone();
                }
            }
            track.map(|track| models::Track { tempo: (analysis.bpm * 100.0) as u32, key: analysis.key.clone(), ..track.clone() })
        })
        .map(|(stored, held)| (stored, held, detail))
        .map_err(|e| format!("result not stored: {e}"))
    });
    match stored {
        Ok((stored, held, detail)) => {
            sink.log(
                LogLevel::Ok,
                format!(
                    "[{completed}/{total}] {file_name} — bpm={:.1} key={} beats={} cues={} {detail}",
                    stored.bpm,
                    stored.key,
                    stored.beat_grid.beats.len(),
                    stored.cue_points.len(),
                ),
            );
            sink.log(
                LogLevel::Dim,
                format!(
                    "[{completed}/{total}] {file_name} DB+ANLZ write {:.1}s, library lock held {:.1}ms",
                    t_db.elapsed().as_secs_f64(),
                    held.as_secs_f64() * 1000.0
                ),
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

fn store_analysis(
    shared: &SharedLibrary,
    track_id: i64,
    source_path: &Path,
    analysis: &models::AnalysisResult,
    stages: u8,
    prepare: impl Fn(Option<&models::Track>, &mut models::AnalysisResult) -> Option<models::Track>,
) -> Result<(models::AnalysisResult, Duration), String> {
    let mut prepared = analysis.clone();
    let (write, track_row, read_held) = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let held = Instant::now();
        let track = lib.get_track(track_id).map_err(|e| e.to_string())?;
        if track.as_ref().is_some_and(|track| track.source_path != source_path) {
            return Err(format!("track {track_id} no longer points at {}", source_path.display()));
        }
        let track_row = prepare(track.as_ref(), &mut prepared);
        let write = lib.begin_analysis(track_id, &prepared, stages).map_err(|e| e.to_string())?;
        (write, track_row, held.elapsed())
    };
    let staged = write.stage().map_err(|e| e.to_string())?;
    let lib = shared.lock().map_err(|e| e.to_string())?;
    let held = Instant::now();
    if !lib.commit_analysis(staged, track_row.as_ref()).map_err(|e| e.to_string())? {
        prepared = analysis.clone();
        let track = lib.get_track(track_id).map_err(|e| e.to_string())?;
        if let Some(track) = prepare(track.as_ref(), &mut prepared) {
            lib.update_track(track_id, &track).map_err(|e| e.to_string())?;
        }
        lib.set_analysis_stages(track_id, &prepared, stages).map_err(|e| e.to_string())?;
    }
    Ok((prepared, read_held + held.elapsed()))
}

fn store_fast_pass(
    shared: &SharedLibrary,
    sink: &Arc<dyn AnalysisSink>,
    track_id: i64,
    source_path: &Path,
    result: &models::AnalysisResult,
    detail: &str,
) -> Result<(), String> {
    store_analysis(shared, track_id, source_path, result, FAST_STAGES, |track, analysis| {
        track.filter(|_| !analysis.key.is_empty()).map(|track| models::Track { key: analysis.key.clone(), ..track.clone() })
    })?;
    sink.log(LogLevel::Dim, format!("{} waveform+key stored key={} {detail}", file_name_of(source_path), result.key));
    sink.partial(track_id);
    Ok(())
}

fn partial_store(shared: &SharedLibrary, sink: &Arc<dyn AnalysisSink>, track_id: i64, source_path: &Path, started: Instant) -> PartialStore {
    let (shared, sink, file_name, source_path) = (shared.clone(), sink.clone(), file_name_of(source_path), source_path.to_path_buf());
    Arc::new(move |partial| {
        let stages = if partial.key.is_empty() { analysis_stage::WAVEFORM } else { FAST_STAGES };
        let stored = store_analysis(&shared, track_id, &source_path, &partial, stages, |track, analysis| {
            track.filter(|_| stages == FAST_STAGES).map(|track| models::Track { key: analysis.key.clone(), ..track.clone() })
        });
        match stored {
            Ok(_) => {
                sink.log(LogLevel::Dim, format!("{file_name} waveform ready key={} {:.1}s", partial.key, started.elapsed().as_secs_f64()));
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
#[allow(clippy::too_many_arguments)]
async fn run_now_lane(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    cancel: Arc<AtomicBool>,
    runner: Runner,
    wide_threads: usize,
    sink: Arc<dyn AnalysisSink>,
    detail_dir: Option<PathBuf>,
) -> Result<(), String> {
    let (track_id, source_path) = item;
    let _guard = NowGuard { coordinator: coordinator.clone(), track_id };
    let started = Instant::now();
    let on_partial = partial_store(&shared, &sink, track_id, &source_path, started);
    let outcome = match runner {
        Runner::Rust { engine } => {
            sink.log(LogLevel::Info, format!("Priority: {} on the now lane, threads={wide_threads}", file_name_of(&source_path)));
            let job = JobControl { cancel, pause: Arc::new(AtomicBool::new(false)), on_partial: Some(on_partial), detail_cache: detail_dir };
            analyze_one_track_rust(engine, source_path.clone(), wide_threads, Pass::All, job).await
        }
        Runner::Python { command } => {
            sink.log(LogLevel::Info, format!("Priority: {} starts at once, background paused", file_name_of(&source_path)));
            let bands_path = source_path.clone();
            let bands = tokio::task::spawn_blocking(move || {
                crate::analysis_priority::run_below_ui(|| {
                    if let Ok((waves, detail)) = crate::band_analysis::measure_waveforms_and_detail(&bands_path) {
                        on_partial(fourfour_analysis::service::waveform_only_result(&waves));
                        if let Some(dir) = &detail_dir {
                            if let Err(e) = crate::waveform_detail::store_detail(dir, &bands_path, &detail) {
                                eprintln!("waveform detail of {} not cached: {e}", bands_path.display());
                            }
                        }
                    }
                })
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
    let resolve = || resolve_runner(&env, env.locate_weights(), &coordinator.analyzers, resolve_python_checked).map(|(runner, _)| runner);
    prioritize_with_resolver(shared, coordinator.clone(), item, sink, env.clone(), resolve).await
}

async fn prioritize_with_runner(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
    runner: Runner,
) -> Result<(), String> {
    prioritize_with_resolver(shared, coordinator, item, sink, env, || Ok(runner)).await
}

async fn prioritize_with_resolver(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    item: AnalysisWorkItem,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
    resolve: impl FnOnce() -> Result<Runner, String>,
) -> Result<(), String> {
    let runner = resolve().inspect_err(|_| {
        if !coordinator.is_active() {
            sink.complete();
        }
    })?;
    let wide_threads = match &runner {
        Runner::Rust { engine } => engine.plan(1).threads_per_track,
        Runner::Python { .. } => 0,
    };
    match coordinator.claim_now(item.0, wide_threads) {
        NowClaim::AlreadyRunning => Ok(()),
        NowClaim::NoRun => run_with_runner(shared, coordinator, vec![item], sink, env, Some(runner)).await.map(|_| ()),
        NowClaim::Granted { cancel } => {
            let detail_dir = env.app_data_dir.as_ref().map(|dir| dir.join(DETAIL_CACHE_DIR));
            run_now_lane(shared, coordinator, item, cancel, runner, wide_threads, sink, detail_dir).await
        }
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

pub(crate) async fn run_with_runner(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
    sink: Arc<dyn AnalysisSink>,
    env: BackendEnv,
    chosen_runner: Option<Runner>,
) -> Result<RunOutcome, String> {
    let resolve = || match chosen_runner {
        Some(runner) => Ok((runner, Vec::new())),
        None => resolve_runner(&env, env.locate_weights(), &coordinator.analyzers, resolve_python_checked),
    };
    run_with_resolver(shared, coordinator.clone(), pending, sink, env.app_data_dir.clone(), resolve).await
}

async fn run_with_resolver(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
    sink: Arc<dyn AnalysisSink>,
    app_data_dir: Option<PathBuf>,
    resolve: impl FnOnce() -> Result<(Runner, Vec<String>), String>,
) -> Result<RunOutcome, String> {
    if pending.is_empty() {
        sink.log(LogLevel::Info, "No unanalyzed tracks — nothing to do".to_string());
        return Ok(RunOutcome::NothingToDo);
    }

    while !coordinator.try_begin() {
        if coordinator.merge_work(&pending, false).is_some() {
            return Ok(RunOutcome::Merged);
        }
        tokio::time::sleep(SCHEDULER_POLL).await;
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

    let outcome = run_registered(shared, coordinator, sink.clone(), app_data_dir, resolve, initial_total).await;
    if outcome.is_err() {
        sink.complete();
    }
    outcome
}

async fn run_registered(
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    sink: Arc<dyn AnalysisSink>,
    app_data_dir: Option<PathBuf>,
    resolve: impl FnOnce() -> Result<(Runner, Vec<String>), String>,
    initial_total: u32,
) -> Result<RunOutcome, String> {
    let (runner, notes) = resolve()?;
    for note in notes {
        sink.log(LogLevel::Info, note);
    }
    let (max_in_flight, model_jobs) = match &runner {
        Runner::Python { command } => {
            sink.log(LogLevel::Info, format!("Starting analysis: {initial_total} tracks, python={command}"));
            (PYTHON_MAX_IN_FLIGHT, ModelJobs { max: PYTHON_MAX_IN_FLIGHT, whole: 0 })
        }
        Runner::Rust { engine } => {
            let plan = engine.plan(usize::MAX);
            let model_jobs = (engine.plan(1).threads_per_track / MODEL_THREADS_PER_JOB).max(MIN_MODEL_JOBS);
            sink.log(
                LogLevel::Info,
                format!(
                    "Starting analysis: {initial_total} tracks, rust workers={} threads/track={} model jobs={model_jobs}",
                    plan.workers, plan.threads_per_track
                ),
            );
            (plan.workers, ModelJobs { max: model_jobs, whole: coordinator.whole_jobs.unwrap_or(WHOLE_JOBS_DURING_FAST_PASSES.min(plan.workers.saturating_sub(1).max(1))) })
        }
    };

    let mut join_set: tokio::task::JoinSet<(i64, PathBuf, Stage, TrackResult)> = tokio::task::JoinSet::new();

    let staged = coordinator.staged && matches!(runner, Runner::Rust { .. });
    let detail_cache = app_data_dir.as_ref().map(|dir| dir.join(DETAIL_CACHE_DIR));
    loop {
        while join_set.len() < max_in_flight && !coordinator.is_cancelled() && !coordinator.background_paused() {
            let stored_stages = |id: i64| shared.lock().ok().and_then(|lib| lib.analysis_stages(id).ok().flatten()).unwrap_or(0);
            let task = coordinator.take_next(staged, stored_stages, model_jobs, |jobs_left| match &runner {
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
                    let on_partial = matches!(task.stage, Stage::Fast | Stage::Whole)
                        .then(|| partial_store(&shared, &sink, task.track_id, &task.source_path, Instant::now()));
                    let job = JobControl { cancel: task.cancel, pause: coordinator.pause_flag(), on_partial, detail_cache: detail_cache.clone() };
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

        if coordinator.is_preempted(track_id) {
            coordinator.finish_background(track_id);
            continue;
        }
        match (stage, analysis_result) {
            (Stage::Fast, Ok((result, detail))) => match store_fast_pass(&shared, &sink, track_id, &source_path, &result, &detail) {
                Ok(()) => coordinator.hand_to_grid((track_id, source_path)),
                Err(e) => {
                    record_outcome(&shared, &sink, &coordinator, track_id, &source_path, stage, Err(format!("result not stored: {e}")))?;
                    coordinator.finish_background(track_id);
                }
            },
            (_, outcome) => {
                record_outcome(&shared, &sink, &coordinator, track_id, &source_path, stage, outcome)?;
                coordinator.finish_background(track_id);
            }
        }
    }

    let completed = coordinator.completed.load(Ordering::Relaxed);
    let errors = coordinator.errors.load(Ordering::Relaxed);
    let total = coordinator.total();
    let (ui_pauses, ui_paused) = coordinator.ui_pause_totals();
    sink.log(
        LogLevel::Info,
        format!("Analysis complete: {completed} done, {errors} errors out of {total}; held for the UI {ui_pauses}x, {:.1}s", ui_paused.as_secs_f64()),
    );
    sink.complete();
    Ok(RunOutcome::Finished { errors, total })
}

#[cfg(test)]
#[path = "analysis_run_tests.rs"]
mod tests;
