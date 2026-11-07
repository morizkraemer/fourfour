use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, WaveformPreview};
use pioneer_usb_writer::waveform::Waveforms;

use crate::analyze::{analyze_controlled, Analysis, Config, Control};
use crate::bt::{chunk_frames, BeatEngine, BeatStatus};
use crate::keymap::camelot;

pub const MODELS_ENV: &str = "FOURFOUR_MODELS_DIR";
pub const MEL_MODEL: &str = "mel_spectrogram.onnx";
pub const BEAT_MODEL: &str = "beat_this.onnx";
pub const BEAT_MODEL_ENV: &str = "FOURFOUR_BEAT_MODEL";

pub fn beat_model() -> String {
    std::env::var(BEAT_MODEL_ENV).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| BEAT_MODEL.to_string())
}

/// Packs the GEMM weights once at load instead of on every chunk. It pays for the int8 beat model (R150: -4% CPU,
/// +25 MB, same output) and not for the fp32 default (no gain, +90-130 MB), so only a non-default model file gets it.
pub fn prepacks_weights(beat_file: &str) -> bool {
    beat_file != BEAT_MODEL
}

pub fn models_present(dir: &Path) -> bool {
    dir.join(MEL_MODEL).is_file() && dir.join(beat_model()).is_file()
}

pub const ANALYSIS_VERSION: u32 = 2;

pub fn analysis_key(models_dir: Option<&Path>) -> String {
    let len = |name: &str| models_dir.and_then(|dir| std::fs::metadata(dir.join(name)).ok()).map_or(0, |m| m.len());
    format!("analysis-{ANALYSIS_VERSION}-{}-{}", len(MEL_MODEL), len(&beat_model()))
}

pub fn dev_models_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototyping/analysis-packaging/models")
}

pub fn model_search_dirs(env_dir: Option<PathBuf>, app_data_dir: Option<&Path>, resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(env_dir);
    if let Some(dir) = app_data_dir {
        candidates.push(dir.join("models"));
    }
    if let Some(dir) = resource_dir {
        candidates.push(dir.join("models"));
    }
    candidates.push(dev_models_dir());
    candidates
}

pub fn locate_models_in(app_data_dir: Option<&Path>, resource_dir: Option<&Path>) -> Option<PathBuf> {
    model_search_dirs(std::env::var_os(MODELS_ENV).map(PathBuf::from), app_data_dir, resource_dir)
        .into_iter()
        .find(|d| models_present(d))
}

pub fn locate_models(app_data_dir: Option<&Path>) -> Option<PathBuf> {
    locate_models_in(app_data_dir, None)
}

/// Overrides the machine's memory (MB) the analysis plans with, to simulate a smaller machine; available memory is
/// then half of it at most, as on macOS.
pub const MEMORY_ENV: &str = "FOURFOUR_ANALYSIS_MEMORY_MB";

fn simulated_memory_mb() -> Option<f64> {
    std::env::var(MEMORY_ENV).ok().and_then(|v| v.parse::<f64>().ok()).filter(|mb| *mb > 0.0)
}

fn meminfo_mb(field: &str) -> Option<f64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb = text.lines().find(|l| l.starts_with(field))?.split_whitespace().nth(1)?.parse::<f64>().ok()?;
    Some(kb / 1024.0)
}

fn sysctl_memory_mb() -> Option<f64> {
    let out = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    let bytes = String::from_utf8(out.stdout).ok()?.trim().parse::<f64>().ok()?;
    Some(bytes / 1024.0 / 1024.0)
}

pub fn total_memory_mb() -> f64 {
    simulated_memory_mb().or_else(|| meminfo_mb("MemTotal:")).or_else(sysctl_memory_mb).unwrap_or(8192.0)
}

pub fn available_memory_mb() -> f64 {
    if let Some(mb) = simulated_memory_mb() {
        return (mb * 0.5).min(meminfo_mb("MemAvailable:").unwrap_or(f64::INFINITY));
    }
    meminfo_mb("MemAvailable:").or_else(|| sysctl_memory_mb().map(|mb| mb * 0.5)).unwrap_or(4096.0)
}

/// Share of the machine's memory the analysis plans to hold: 2 GB on an 8 GB Mac.
const ANALYSIS_MEMORY_FRACTION: f64 = 0.25;
/// Model weights and the decoded audio of the tracks in flight, before any beat-model chunk runs.
const ANALYSIS_BASE_MB: f64 = 600.0;
/// Activations of one beat-model chunk while it runs (rten builds a fresh buffer pool for every run): about 400 MB
/// for the 1500 frames CoreML takes (R57), about 120 MB for the 500 frames rten takes (R108, peak RSS against the chunk
/// threads of a batch). The attention matrices grow with the square of the length, so between the two sizes the
/// estimate is a straight line through both.
const CHUNK_ACTIVATION_MB_AT_1500: f64 = 400.0;
const CHUNK_ACTIVATION_MB_AT_500: f64 = 120.0;

fn chunk_activation_mb(frames: usize) -> f64 {
    let share = ((frames as f64 - 500.0) / 1000.0).max(0.0);
    CHUNK_ACTIVATION_MB_AT_500 + (CHUNK_ACTIVATION_MB_AT_1500 - CHUNK_ACTIVATION_MB_AT_500) * share
}

const CHUNK_BUDGET_CEILING_COREML_MB: u64 = 2048;
const CHUNK_BUDGET_CEILING_CPU_MB: u64 = 4096;

fn default_chunk_budget_ceiling_mb(runtime: &str) -> u64 {
    if runtime == "coreml" {
        CHUNK_BUDGET_CEILING_COREML_MB
    } else {
        CHUNK_BUDGET_CEILING_CPU_MB
    }
}

fn chunk_budget_ceiling_mb() -> Option<u64> {
    static CEILING: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *CEILING.get_or_init(|| match std::env::var("FOURFOUR_CHUNK_BUDGET_MB") {
        Ok(v) => Some(v.trim().parse().unwrap_or_else(|_| panic!("FOURFOUR_CHUNK_BUDGET_MB must be a whole number of MB, got {v:?}"))),
        Err(_) => Some(default_chunk_budget_ceiling_mb(crate::bt::beat_runtime())),
    })
}

/// Beat-model chunks that may run at once: what fits into a quarter of the machine's memory, and into `mem_fraction`
/// of what is available now, and under the chunk budget ceiling (2048 MB with CoreML, 4096 MB otherwise, `FOURFOUR_CHUNK_BUDGET_MB` overrides). At least one,
/// at most `max_threads`.
pub fn chunk_slots(max_threads: usize, mem_fraction: f64, total_mb: f64, available_mb: f64) -> usize {
    chunk_slots_with_ceiling(max_threads, mem_fraction, total_mb, available_mb, chunk_budget_ceiling_mb(), chunk_activation_mb(chunk_frames()))
}

fn chunk_slots_with_ceiling(max_threads: usize, mem_fraction: f64, total_mb: f64, available_mb: f64, ceiling_mb: Option<u64>, activation_mb: f64) -> usize {
    let budget = (total_mb * ANALYSIS_MEMORY_FRACTION).min(available_mb * mem_fraction);
    let mut slots = ((budget - ANALYSIS_BASE_MB) / activation_mb).floor().max(1.0) as usize;
    if let Some(ceiling) = ceiling_mb {
        slots = slots.min((ceiling as f64 / activation_mb).floor().max(1.0) as usize);
    }
    slots.min(max_threads.max(1))
}

/// Caps how many tracks the analysis works on at once (default: as many as it has threads).
pub const WORKERS_ENV: &str = "FOURFOUR_ANALYSIS_WORKERS";

#[derive(Clone, Debug)]
pub struct PoolOptions {
    pub max_threads: usize,
    pub max_workers: Option<usize>,
    pub mem_fraction: f64,
    pub rss_mb_per_worker: f64,
}

impl Default for PoolOptions {
    fn default() -> Self {
        PoolOptions {
            max_threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
            max_workers: std::env::var(WORKERS_ENV).ok().and_then(|v| v.trim().parse().ok()),
            mem_fraction: 0.5,
            rss_mb_per_worker: 900.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolPlan {
    pub workers: usize,
    pub threads_per_track: usize,
}

pub fn plan_pool(opts: &PoolOptions, available_mb: f64, jobs: usize) -> PoolPlan {
    let by_memory = ((available_mb * opts.mem_fraction) / opts.rss_mb_per_worker).floor().max(1.0) as usize;
    let mut workers = opts.max_threads.max(1).min(by_memory).min(jobs.max(1));
    if let Some(cap) = opts.max_workers {
        workers = workers.min(cap.max(1));
    }
    PoolPlan { workers, threads_per_track: (opts.max_threads / workers).max(1) }
}

/// Which stages one run computes. The waveform is cheap, so `Grid` recomputes it from the same decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    Fast,
    Grid,
    All,
}

pub struct Analyzer {
    engine: Arc<BeatEngine>,
    cfg: Config,
    pool: PoolOptions,
}

impl Analyzer {
    pub fn load(models_dir: &Path, pool: PoolOptions) -> Result<Self> {
        let beat_file = beat_model();
        let engine = BeatEngine::load(models_dir, &beat_file, prepacks_weights(&beat_file))?;
        engine.share_chunk_threads(chunk_slots(pool.max_threads, pool.mem_fraction, total_memory_mb(), available_memory_mb()));
        Ok(Analyzer { engine: Arc::new(engine), cfg: Config::default(), pool })
    }

    pub fn with_config(mut self, edit: impl FnOnce(&mut Config)) -> Self {
        edit(&mut self.cfg);
        self
    }

    pub fn plan(&self, jobs: usize) -> PoolPlan {
        plan_pool(&self.pool, available_memory_mb(), jobs)
    }

    pub fn beat_status(&self) -> BeatStatus {
        self.engine.beat_status()
    }

    pub fn analyze(&self, path: &Path) -> Result<AnalysisResult> {
        self.analyze_with(path, &AtomicBool::new(false), self.plan(usize::MAX).threads_per_track)
    }

    pub fn analyze_with(&self, path: &Path, cancel: &AtomicBool, threads: usize) -> Result<AnalysisResult> {
        self.analyze_controlled(path, &Control::new(cancel), threads, Pass::All, None)
    }

    pub fn analyze_controlled(
        &self,
        path: &Path,
        control: &Control,
        threads: usize,
        pass: Pass,
        on_partial: Option<&(dyn Fn(AnalysisResult) + Sync)>,
    ) -> Result<AnalysisResult> {
        let partial = on_partial.map(|emit| {
            move |w: &Waveforms, key: Option<usize>| {
                let mut result = waveform_only_result(w);
                result.key = key.map(camelot).unwrap_or_default().to_string();
                emit(result)
            }
        });
        let control = Control {
            cancel: control.cancel,
            pause: control.pause,
            on_waveform: partial.as_ref().map(|f| f as &(dyn Fn(&Waveforms, Option<usize>) + Sync)),
        };
        let mut cfg = self.cfg.clone();
        cfg.threads = threads.max(1);
        cfg.beats = pass != Pass::Fast;
        cfg.key = cfg.key && pass != Pass::Grid;
        let analysis = analyze_controlled(&self.engine, path, &cfg, &control)?;
        Ok(to_app_result(&analysis))
    }
}

static DEFAULT_ANALYZER: OnceLock<std::result::Result<Analyzer, String>> = OnceLock::new();

pub fn analyze(path: &Path) -> Result<AnalysisResult> {
    let analyzer = DEFAULT_ANALYZER
        .get_or_init(|| {
            let dir = locate_models(None).ok_or_else(|| format!("model weights not found (set {MODELS_ENV})"))?;
            Analyzer::load(&dir, PoolOptions::default()).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow!("{e}"))?;
    analyzer.analyze(path)
}

pub struct BatchEvent<'a> {
    pub index: usize,
    pub done: usize,
    pub total: usize,
    pub path: &'a Path,
    pub result: &'a Result<AnalysisResult>,
}

pub fn run_pool<J, N, D>(analyzer: &Analyzer, plan: PoolPlan, cancel: &AtomicBool, next: N, done: D)
where
    J: Send,
    N: Fn() -> Option<(J, PathBuf)> + Sync,
    D: Fn(J, Result<AnalysisResult>) + Sync,
{
    std::thread::scope(|s| {
        for _ in 0..plan.workers {
            s.spawn(|| {
                while !cancel.load(Ordering::Relaxed) {
                    let Some((job, path)) = next() else { break };
                    done(job, analyzer.analyze_with(&path, cancel, plan.threads_per_track));
                }
            });
        }
    });
}

pub fn analyze_batch(
    analyzer: &Analyzer,
    paths: &[PathBuf],
    cancel: &AtomicBool,
    on_progress: impl Fn(&BatchEvent) + Sync,
) -> Vec<Option<Result<AnalysisResult>>> {
    let cursor = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<AnalysisResult>>>> = Mutex::new(paths.iter().map(|_| None).collect());
    run_pool(
        analyzer,
        analyzer.plan(paths.len()),
        cancel,
        || {
            let i = cursor.fetch_add(1, Ordering::SeqCst);
            paths.get(i).map(|p| (i, p.clone()))
        },
        |index, result| {
            let done = finished.fetch_add(1, Ordering::SeqCst) + 1;
            on_progress(&BatchEvent { index, done, total: paths.len(), path: &paths[index], result: &result });
            results.lock().unwrap()[index] = Some(result);
        },
    );
    results.into_inner().unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarOneRule {
    ModelVote,
    FirstBeat,
    FirstBeatUnlessModelStrong,
}

pub const BAR_ONE_RULE: BarOneRule = BarOneRule::FirstBeatUnlessModelStrong;
pub const MODEL_STRONG_SHARE: f64 = 0.8;

pub fn bar_phase(beats_s: &[f64], downbeats_s: &[f32], delay_s: f64) -> usize {
    bar_phase_by(BAR_ONE_RULE, beats_s, downbeats_s, delay_s)
}

pub fn bar_phase_by(rule: BarOneRule, beats_s: &[f64], downbeats_s: &[f32], delay_s: f64) -> usize {
    if rule == BarOneRule::FirstBeat || beats_s.len() < 2 {
        return 0;
    }
    let period = (beats_s[beats_s.len() - 1] - beats_s[0]) / (beats_s.len() - 1) as f64;
    let mut votes = [0usize; 4];
    for &d in downbeats_s {
        let t = d as f64 + delay_s;
        let after = beats_s.partition_point(|&b| b < t);
        let nearest = [after.checked_sub(1), (after < beats_s.len()).then_some(after)]
            .into_iter()
            .flatten()
            .min_by(|&a, &b| (beats_s[a] - t).abs().total_cmp(&(beats_s[b] - t).abs()))
            .unwrap();
        if (beats_s[nearest] - t).abs() < period / 2.0 {
            votes[nearest % 4] += 1;
        }
    }
    let top = (0..4).max_by_key(|&p| (votes[p], std::cmp::Reverse(p))).unwrap();
    let total: usize = votes.iter().sum();
    match rule {
        BarOneRule::FirstBeatUnlessModelStrong if (votes[top] as f64) < MODEL_STRONG_SHARE * total.max(1) as f64 => 0,
        _ => top,
    }
}

pub fn beats_from_grid(beats_s: &[f64], downbeats_s: &[f32], delay_s: f64, bpm: f64) -> Vec<Beat> {
    let phase = bar_phase(beats_s, downbeats_s, delay_s);
    let tempo = (bpm * 100.0).round() as u32;
    beats_s
        .iter()
        .enumerate()
        .map(|(i, &t)| Beat {
            bar_position: ((i + 4 - phase) % 4) as u8 + 1,
            time_ms: (t * 1000.0).round() as u32,
            tempo,
        })
        .collect()
}

pub fn waveform_only_result(w: &Waveforms) -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: Vec::new() },
        waveform: w.preview().unwrap_or(WaveformPreview { data: [0; 400] }),
        bpm: 0.0,
        key: String::new(),
        energy: None,
        cue_points: Vec::new(),
        color_waveform: Some(w.color_waveform()),
    }
}

pub fn to_app_result(a: &Analysis) -> AnalysisResult {
    let bpm = a.bpm.unwrap_or(0.0);
    let (waveform, color_waveform) = match &a.waveforms {
        Some(w) => (w.preview().unwrap_or(WaveformPreview { data: [0; 400] }), Some(w.color_waveform())),
        None => (WaveformPreview { data: [0; 400] }, None),
    };
    AnalysisResult {
        beat_grid: BeatGrid { beats: beats_from_grid(&a.beats_s, &a.downbeats_s, a.mp3_delay_s, bpm) },
        waveform,
        bpm,
        key: a.key_class.map(camelot).unwrap_or_default().to_string(),
        energy: None,
        cue_points: Vec::new(),
        color_waveform,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waveform::Waveforms;

    fn grid(first: f64, period: f64, n: usize) -> Vec<f64> {
        (0..n).map(|i| first + i as f64 * period).collect()
    }

    fn analysis(beats: Vec<f64>, downbeats: Vec<f32>) -> Analysis {
        Analysis {
            truncated: false,
            duration_s: 10.0,
            bpm: Some(128.0),
            first_beat_s: beats.first().copied(),
            beats_s: beats,
            raw_beats_s: Vec::new(),
            downbeats_s: downbeats,
            key_class: Some(0),
            key_confidence: 0.5,
            waveforms: Some(Waveforms { pwav: vec![7; 400], pwv7: vec![[1, 2, 3]; 10], pwv6: vec![[4, 5, 6]; 10], ..Waveforms::default() }),
            waveform_checksum: None,
            detail_entries: 10,
            mp3_delay_s: 0.0,
            logits: None,
            ms: serde_json::Value::Null,
        }
    }

    #[test]
    fn downbeats_set_bar_position_phase() {
        let beats = grid(0.1, 0.5, 12);
        let downbeats = vec![beats[2] as f32, beats[6] as f32, beats[10] as f32];
        let mapped = beats_from_grid(&beats, &downbeats, 0.0, 120.0);
        let positions: Vec<u8> = mapped.iter().map(|b| b.bar_position).collect();
        assert_eq!(positions, vec![3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2]);
    }

    #[test]
    fn rules_pick_the_expected_phase_for_a_split_and_a_unanimous_vote() {
        let beats = grid(0.1, 0.5, 40);
        let at = |idx: &[usize]| -> Vec<f32> { idx.iter().map(|&i| beats[i] as f32).collect() };
        let split = at(&[2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 1, 5, 9, 13]);
        let unanimous = at(&[2, 6, 10, 14, 18, 22, 26, 30, 34, 38]);
        assert_eq!(bar_phase_by(BarOneRule::ModelVote, &beats, &split, 0.0), 2);
        assert_eq!(bar_phase_by(BarOneRule::FirstBeat, &beats, &split, 0.0), 0);
        assert_eq!(bar_phase_by(BarOneRule::FirstBeatUnlessModelStrong, &beats, &split, 0.0), 0);
        assert_eq!(bar_phase_by(BarOneRule::FirstBeatUnlessModelStrong, &beats, &unanimous, 0.0), 2);
        assert_eq!(bar_phase_by(BarOneRule::FirstBeat, &beats, &unanimous, 0.0), 0);
    }

    #[test]
    fn missing_downbeats_default_to_first_beat_as_one() {
        let mapped = beats_from_grid(&grid(0.0, 0.5, 5), &[], 0.0, 120.0);
        let positions: Vec<u8> = mapped.iter().map(|b| b.bar_position).collect();
        assert_eq!(positions, vec![1, 2, 3, 4, 1]);
    }

    #[test]
    fn beats_carry_millisecond_time_and_bpm_times_100() {
        let mapped = beats_from_grid(&grid(0.0004, 0.46875, 3), &[], 0.0, 128.0);
        assert_eq!(mapped[0].time_ms, 0);
        assert_eq!(mapped[1].time_ms, 469);
        assert!(mapped.iter().all(|b| b.tempo == 12800));
    }

    #[test]
    fn result_maps_key_waveforms_and_leaves_cues_empty() {
        let r = to_app_result(&analysis(grid(0.0, 0.5, 4), vec![0.0]));
        assert_eq!(r.key, "8B");
        assert_eq!(r.bpm, 128.0);
        assert_eq!(r.waveform.data, [7; 400]);
        let cw = r.color_waveform.unwrap();
        assert_eq!((cw.detail.len(), cw.overview[0]), (10, [4, 5, 6]));
        assert!(cw.exact.is_some());
        assert!(r.cue_points.is_empty() && r.energy.is_none());
        assert_eq!(r.beat_grid.beats.len(), 4);
    }

    #[test]
    fn failed_fit_maps_to_empty_grid_and_zero_bpm() {
        let mut a = analysis(Vec::new(), Vec::new());
        a.bpm = None;
        a.key_class = None;
        let r = to_app_result(&a);
        assert_eq!((r.bpm, r.key.as_str(), r.beat_grid.beats.len()), (0.0, "", 0));
    }

    #[test]
    fn pool_plan_is_capped_by_memory_threads_and_jobs() {
        let opts = PoolOptions { max_threads: 8, max_workers: None, mem_fraction: 0.5, rss_mb_per_worker: 900.0 };
        assert_eq!(plan_pool(&opts, 64_000.0, 100), PoolPlan { workers: 8, threads_per_track: 1 });
        assert_eq!(plan_pool(&opts, 3_700.0, 100), PoolPlan { workers: 2, threads_per_track: 4 });
        assert_eq!(plan_pool(&opts, 500.0, 100), PoolPlan { workers: 1, threads_per_track: 8 });
        assert_eq!(plan_pool(&opts, 64_000.0, 3), PoolPlan { workers: 3, threads_per_track: 2 });
    }

    #[test]
    fn only_a_non_default_beat_model_prepacks_its_weights() {
        assert!(!prepacks_weights(BEAT_MODEL));
        assert!(prepacks_weights("beat_this_q8sc.onnx"));
    }

    #[test]
    fn chunk_slots_fit_the_memory_budget_and_the_threads() {
        let slots = |threads, total, available| chunk_slots_with_ceiling(threads, 0.5, total, available, None, 400.0);
        assert_eq!(slots(7, 8192.0, 4096.0), 3);
        assert_eq!(slots(9, 16384.0, 8192.0), 8);
        assert_eq!(slots(15, 65536.0, 32000.0), 15);
        assert_eq!(slots(15, 65536.0, 1500.0), 1);
        assert_eq!(slots(1, 65536.0, 32000.0), 1);
    }

    #[test]
    fn chunk_activation_follows_the_chunk_length() {
        assert_eq!(chunk_activation_mb(1500), 400.0);
        assert_eq!(chunk_activation_mb(500), 120.0);
        assert_eq!(chunk_activation_mb(100), 120.0);
        assert_eq!(chunk_activation_mb(1000), 260.0);
    }

    #[test]
    fn short_chunks_leave_room_for_every_thread_on_a_small_machine() {
        let slots = |activation| chunk_slots_with_ceiling(6, 0.5, 8192.0, 4096.0, None, activation);
        assert_eq!(slots(chunk_activation_mb(1500)), 3);
        assert_eq!(slots(chunk_activation_mb(500)), 6);
    }

    #[test]
    fn chunk_budget_ceiling_caps_the_slots_and_never_raises_them() {
        let slots = |ceiling| chunk_slots_with_ceiling(15, 0.5, 65536.0, 32000.0, ceiling, 400.0);
        assert_eq!(slots(None), 15);
        assert_eq!(slots(Some(2048)), 5);
        assert_eq!(slots(Some(1024)), 2);
        assert_eq!(slots(Some(100)), 1);
        assert_eq!(slots(Some(0)), 1);
        assert_eq!(slots(Some(1_000_000)), 15);
        assert_eq!(chunk_slots_with_ceiling(7, 0.5, 8192.0, 4096.0, Some(4096), 400.0), 3);
        assert_eq!(chunk_slots_with_ceiling(15, 0.5, 65536.0, 1500.0, Some(4096), 400.0), 1);
    }

    #[test]
    fn default_ceiling_is_2048_with_coreml_and_4096_elsewhere() {
        assert_eq!(default_chunk_budget_ceiling_mb("coreml"), 2048);
        assert_eq!(default_chunk_budget_ceiling_mb("rten"), 4096);
        assert_eq!(default_chunk_budget_ceiling_mb("ort-cpu"), 4096);
        assert_eq!(chunk_slots_with_ceiling(15, 0.5, 65536.0, 32000.0, Some(default_chunk_budget_ceiling_mb("rten")), 400.0), 10);
        assert_eq!(chunk_slots_with_ceiling(15, 0.5, 65536.0, 32000.0, Some(default_chunk_budget_ceiling_mb("coreml")), 400.0), 5);
    }

    #[test]
    fn model_search_order_is_env_then_app_data_then_resources_then_dev() {
        let dirs = model_search_dirs(Some(PathBuf::from("/env")), Some(Path::new("/data")), Some(Path::new("/res")));
        assert_eq!(dirs[..3], [PathBuf::from("/env"), PathBuf::from("/data/models"), PathBuf::from("/res/models")]);
        assert_eq!(dirs[3], dev_models_dir());
        assert_eq!(dirs.len(), 4);
        assert_eq!(model_search_dirs(None, None, None), vec![dev_models_dir()]);
    }

    #[test]
    fn locate_models_requires_both_files() {
        let dir = std::env::temp_dir().join(format!("ff-models-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("models")).unwrap();
        assert!(!models_present(&dir.join("models")));
        std::fs::write(dir.join("models").join(MEL_MODEL), b"x").unwrap();
        std::fs::write(dir.join("models").join(BEAT_MODEL), b"x").unwrap();
        assert!(models_present(&dir.join("models")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn analysis_key_follows_the_model_files() {
        let dir = std::env::temp_dir().join(format!("ff-key-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(analysis_key(None), format!("analysis-{ANALYSIS_VERSION}-0-0"));
        std::fs::write(dir.join(MEL_MODEL), b"xx").unwrap();
        std::fs::write(dir.join(BEAT_MODEL), b"yyy").unwrap();
        let first = analysis_key(Some(&dir));
        assert_eq!(first, format!("analysis-{ANALYSIS_VERSION}-2-3"));
        std::fs::write(dir.join(BEAT_MODEL), b"yyyy").unwrap();
        assert_ne!(analysis_key(Some(&dir)), first);
        std::fs::remove_dir_all(&dir).ok();
    }
}
