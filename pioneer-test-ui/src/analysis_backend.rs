use std::path::{Path, PathBuf};
use std::sync::Arc;

use fourfour_analysis::analyze::Control;
use fourfour_analysis::service::{Analyzer, Pass, PoolOptions, PoolPlan, MODELS_ENV};
use pioneer_usb_writer::models::AnalysisResult;

pub const BACKEND_ENV: &str = "FOURFOUR_ANALYSIS_BACKEND";
pub const THREADS_ENV: &str = "FOURFOUR_ANALYSIS_THREADS";
const CONFIG_KEY: &str = "analysis_backend";
const AUTO_ANALYZE_KEY: &str = "auto_analyze_new_tracks";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisBackend {
    Rust,
    Python,
}

impl AnalysisBackend {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
        }
    }
}

pub struct BackendChoice {
    pub backend: AnalysisBackend,
    pub note: String,
}

pub fn choose_backend(requested: Option<AnalysisBackend>, weights: Option<&Path>) -> BackendChoice {
    match (requested, weights) {
        (Some(AnalysisBackend::Python), _) => BackendChoice {
            backend: AnalysisBackend::Python,
            note: "analysis backend: python (set by setting)".to_string(),
        },
        (Some(AnalysisBackend::Rust), None) => BackendChoice {
            backend: AnalysisBackend::Python,
            note: format!(
                "analysis backend: python — rust requested but model weights are missing \
                 (looked in ${MODELS_ENV}, <app data>/models, dev models dir; run prototyping/analysis-packaging/fetch-models.sh)"
            ),
        },
        (Some(AnalysisBackend::Rust), Some(dir)) => BackendChoice {
            backend: AnalysisBackend::Rust,
            note: format!("analysis backend: rust (weights {})", dir.display()),
        },
        (None, Some(dir)) => BackendChoice {
            backend: AnalysisBackend::Rust,
            note: format!("analysis backend: rust (default, weights {})", dir.display()),
        },
        (None, None) => BackendChoice {
            backend: AnalysisBackend::Python,
            note: "analysis backend: python (default, rust model weights not installed — see prototyping/analysis-packaging/fetch-models.sh)"
                .to_string(),
        },
    }
}

#[derive(Clone, Default)]
pub struct BackendEnv {
    pub config_path: Option<PathBuf>,
    pub app_data_dir: Option<PathBuf>,
    pub override_backend: Option<AnalysisBackend>,
    pub max_threads: Option<usize>,
}

impl BackendEnv {
    pub fn from_process(config_path: Option<PathBuf>, app_data_dir: Option<PathBuf>) -> Self {
        BackendEnv {
            config_path,
            app_data_dir,
            override_backend: std::env::var(BACKEND_ENV).ok().and_then(|v| AnalysisBackend::parse(&v)),
            max_threads: std::env::var(THREADS_ENV).ok().and_then(|v| v.parse().ok()),
        }
    }

    pub fn requested(&self) -> Option<AnalysisBackend> {
        self.override_backend.or_else(|| self.config_path.as_deref().and_then(read_setting))
    }

    pub fn pool_options(&self) -> PoolOptions {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        PoolOptions { max_threads: self.max_threads.unwrap_or_else(|| cores.saturating_sub(1).max(1)), ..PoolOptions::default() }
    }
}

pub fn read_setting(config_path: &Path) -> Option<AnalysisBackend> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config_path).ok()?).ok()?;
    AnalysisBackend::parse(json.get(CONFIG_KEY)?.as_str()?)
}

pub fn write_setting(config_path: &Path, backend: Option<AnalysisBackend>) -> Result<(), String> {
    update_config(config_path, CONFIG_KEY, backend.map(|b| serde_json::Value::String(b.name().to_string())))
}

/// Whether never-analysed tracks are queued at launch (default on); partial analyses always resume.
pub fn read_auto_analyze_new(config_path: &Path) -> bool {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get(AUTO_ANALYZE_KEY)?.as_bool())
        .unwrap_or(true)
}

pub fn write_auto_analyze_new(config_path: &Path, enabled: bool) -> Result<(), String> {
    update_config(config_path, AUTO_ANALYZE_KEY, Some(serde_json::Value::Bool(enabled)))
}

fn update_config(config_path: &Path, key: &str, value: Option<serde_json::Value>) -> Result<(), String> {
    let mut json = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = json.as_object_mut().expect("object checked above");
    match value {
        Some(value) => object.insert(key.to_string(), value),
        None => object.remove(key),
    };
    std::fs::write(config_path, json.to_string()).map_err(|e| e.to_string())
}

#[derive(Clone, Default)]
pub struct AnalyzerCache(Arc<std::sync::Mutex<Option<(PathBuf, Arc<Analyzer>)>>>);

impl AnalyzerCache {
    fn get_or_load(&self, dir: &Path, pool: PoolOptions) -> Result<Arc<Analyzer>, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if let Some((loaded_dir, analyzer)) = slot.as_ref() {
            if loaded_dir == dir {
                return Ok(analyzer.clone());
            }
        }
        let analyzer = Arc::new(Analyzer::load(dir, pool).map_err(|e| format!("loading model weights from {}: {e}", dir.display()))?);
        *slot = Some((dir.to_path_buf(), analyzer.clone()));
        Ok(analyzer)
    }
}

pub type PartialResultSink<'a> = Option<&'a (dyn Fn(AnalysisResult) + Sync)>;

pub trait TrackEngine: Send + Sync + 'static {
    fn plan(&self, jobs: usize) -> PoolPlan;
    fn analyze(&self, path: &Path, control: &Control, threads: usize, pass: Pass, on_partial: PartialResultSink) -> Result<AnalysisResult, String>;
}

impl TrackEngine for Analyzer {
    fn plan(&self, jobs: usize) -> PoolPlan {
        Analyzer::plan(self, jobs)
    }

    fn analyze(&self, path: &Path, control: &Control, threads: usize, pass: Pass, on_partial: PartialResultSink) -> Result<AnalysisResult, String> {
        self.analyze_controlled(path, control, threads, pass, on_partial).map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
pub enum Runner {
    Python { command: String },
    Rust { engine: Arc<dyn TrackEngine> },
}

pub fn resolve_runner(
    env: &BackendEnv,
    weights: Option<PathBuf>,
    cache: &AnalyzerCache,
    python_command: impl FnOnce() -> String,
) -> (Runner, Vec<String>) {
    let choice = choose_backend(env.requested(), weights.as_deref());
    let mut notes = vec![choice.note];
    if choice.backend == AnalysisBackend::Rust {
        let dir = weights.expect("rust backend is only chosen with weights");
        match cache.get_or_load(&dir, env.pool_options()) {
            Ok(engine) => return (Runner::Rust { engine }, notes),
            Err(e) => notes.push(format!("rust analyzer unavailable, falling back to python: {e}")),
        }
    }
    (Runner::Python { command: python_command() }, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ff-backend-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_is_rust_only_when_weights_exist() {
        let dir = PathBuf::from("/models");
        assert_eq!(choose_backend(None, Some(&dir)).backend, AnalysisBackend::Rust);
        assert_eq!(choose_backend(None, None).backend, AnalysisBackend::Python);
    }

    #[test]
    fn explicit_python_wins_and_explicit_rust_without_weights_falls_back_with_a_message() {
        let dir = PathBuf::from("/models");
        assert_eq!(choose_backend(Some(AnalysisBackend::Python), Some(&dir)).backend, AnalysisBackend::Python);
        let fallback = choose_backend(Some(AnalysisBackend::Rust), None);
        assert_eq!(fallback.backend, AnalysisBackend::Python);
        assert!(fallback.note.contains("weights are missing"), "{}", fallback.note);
    }

    #[test]
    fn setting_round_trips_and_keeps_other_config_keys() {
        let cfg = scratch("setting").join("config.json");
        std::fs::write(&cfg, r#"{"library_path":"/lib"}"#).unwrap();
        assert_eq!(read_setting(&cfg), None);
        write_setting(&cfg, Some(AnalysisBackend::Python)).unwrap();
        assert_eq!(read_setting(&cfg), Some(AnalysisBackend::Python));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(json["library_path"], "/lib");
        write_setting(&cfg, None).unwrap();
        assert_eq!(read_setting(&cfg), None);
    }

    #[test]
    fn auto_analyze_new_defaults_on_and_keeps_other_config_keys() {
        let cfg = scratch("auto-analyze").join("config.json");
        assert!(read_auto_analyze_new(&cfg));
        std::fs::write(&cfg, r#"{"library_path":"/lib"}"#).unwrap();
        assert!(read_auto_analyze_new(&cfg));
        write_auto_analyze_new(&cfg, false).unwrap();
        assert!(!read_auto_analyze_new(&cfg));
        write_auto_analyze_new(&cfg, true).unwrap();
        assert!(read_auto_analyze_new(&cfg));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(json["library_path"], "/lib");
    }

    #[test]
    fn runner_falls_back_to_python_when_the_weights_dir_is_unloadable() {
        let env = BackendEnv::default();
        let empty = scratch("empty");
        let (runner, notes) = resolve_runner(&env, Some(empty), &AnalyzerCache::default(), || "python3".to_string());
        assert!(matches!(runner, Runner::Python { .. }));
        assert!(notes.iter().any(|n| n.contains("falling back to python")), "{notes:?}");
    }

    #[test]
    fn runner_falls_back_to_python_without_weights() {
        let env = BackendEnv { override_backend: Some(AnalysisBackend::Rust), ..BackendEnv::default() };
        let (runner, notes) = resolve_runner(&env, None, &AnalyzerCache::default(), || "python3".to_string());
        assert!(matches!(runner, Runner::Python { .. }));
        assert!(notes.iter().any(|n| n.contains("weights are missing")), "{notes:?}");
    }

    #[test]
    #[ignore = "needs model weights and FOURFOUR_PARITY_TRACK=<audio file>"]
    fn rust_bands_equal_the_app_band_pass() {
        let track = PathBuf::from(std::env::var("FOURFOUR_PARITY_TRACK").expect("FOURFOUR_PARITY_TRACK"));
        let weights = fourfour_analysis::service::locate_models(None).expect("model weights");
        let analyzer = Analyzer::load(&weights, PoolOptions::default()).unwrap();
        let rust = analyzer.analyze(&track).unwrap().color_waveform.unwrap();
        let app = crate::band_analysis::measure_waveforms(&track).unwrap();
        assert_eq!(rust.detail.len().abs_diff(app.pwv7.len()), 0);
        let differing = rust.detail.iter().zip(&app.pwv7).filter(|(a, b)| a != b).count();
        assert!(differing * 100 < rust.detail.len(), "{differing} of {} columns differ", rust.detail.len());
    }
}
