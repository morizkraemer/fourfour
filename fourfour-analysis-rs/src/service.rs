use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, WaveformPreview};
use pioneer_usb_writer::waveform::Waveforms;

use crate::analyze::{analyze_controlled, Analysis, Config, Control};
use crate::bt::BeatEngine;
use crate::keymap::camelot;

pub const MODELS_ENV: &str = "FOURFOUR_MODELS_DIR";
pub const MEL_MODEL: &str = "mel_spectrogram.onnx";
pub const BEAT_MODEL: &str = "beat_this.onnx";

pub fn models_present(dir: &Path) -> bool {
    dir.join(MEL_MODEL).is_file() && dir.join(BEAT_MODEL).is_file()
}

pub fn dev_models_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototyping/analysis-packaging/models")
}

pub fn locate_models(app_data_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os(MODELS_ENV) {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(dir) = app_data_dir {
        candidates.push(dir.join("models"));
    }
    candidates.push(dev_models_dir());
    candidates.into_iter().find(|d| models_present(d))
}

pub fn available_memory_mb() -> f64 {
    if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
        if let Some(kb) = text
            .lines()
            .find(|l| l.starts_with("MemAvailable:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
        {
            return kb / 1024.0;
        }
    }
    let total_bytes = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok());
    total_bytes.map_or(4096.0, |b| b / 1024.0 / 1024.0 * 0.5)
}

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
            max_workers: None,
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

pub struct Analyzer {
    engine: Arc<BeatEngine>,
    cfg: Config,
    pool: PoolOptions,
}

impl Analyzer {
    pub fn load(models_dir: &Path, pool: PoolOptions) -> Result<Self> {
        let engine = BeatEngine::load(models_dir, BEAT_MODEL, false)?;
        Ok(Analyzer { engine: Arc::new(engine), cfg: Config::default(), pool })
    }

    pub fn with_config(mut self, edit: impl FnOnce(&mut Config)) -> Self {
        edit(&mut self.cfg);
        self
    }

    pub fn plan(&self, jobs: usize) -> PoolPlan {
        plan_pool(&self.pool, available_memory_mb(), jobs)
    }

    pub fn analyze(&self, path: &Path) -> Result<AnalysisResult> {
        self.analyze_with(path, &AtomicBool::new(false), self.plan(usize::MAX).threads_per_track)
    }

    pub fn analyze_with(&self, path: &Path, cancel: &AtomicBool, threads: usize) -> Result<AnalysisResult> {
        self.analyze_controlled(path, &Control::new(cancel), threads, None)
    }

    pub fn analyze_controlled(
        &self,
        path: &Path,
        control: &Control,
        threads: usize,
        on_partial: Option<&(dyn Fn(AnalysisResult) + Sync)>,
    ) -> Result<AnalysisResult> {
        let partial = on_partial.map(|emit| move |w: &Waveforms| emit(waveform_only_result(w)));
        let control = Control {
            cancel: control.cancel,
            pause: control.pause,
            on_waveform: partial.as_ref().map(|f| f as &(dyn Fn(&Waveforms) + Sync)),
        };
        let mut cfg = self.cfg.clone();
        cfg.threads = threads.max(1);
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

pub fn bar_phase(beats_s: &[f64], downbeats_s: &[f32], delay_s: f64) -> usize {
    if beats_s.len() < 2 {
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
    (0..4).max_by_key(|&p| (votes[p], std::cmp::Reverse(p))).unwrap()
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
    fn locate_models_requires_both_files() {
        let dir = std::env::temp_dir().join(format!("ff-models-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("models")).unwrap();
        assert!(!models_present(&dir.join("models")));
        std::fs::write(dir.join("models").join(MEL_MODEL), b"x").unwrap();
        std::fs::write(dir.join("models").join(BEAT_MODEL), b"x").unwrap();
        assert!(models_present(&dir.join("models")));
        std::fs::remove_dir_all(&dir).ok();
    }
}
