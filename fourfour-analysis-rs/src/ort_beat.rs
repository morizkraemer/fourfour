use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};
use beat_this::Tensor;
use ort::environment::Environment;
use ort::session::builder::{GraphOptimizationLevel, PrepackedWeights, SessionBuilder};
use ort::session::Session;
use ort::value::TensorRef;

/// The beat model on onnxruntime: with CoreML (GPU) on macOS builds with the `coreml` feature, on the CPU otherwise.
/// `run` takes a session mutably, so every chunk thread needs its own; `open_sessions` opens them one after the other
/// on a background thread (with CoreML the first open after a model change compiles the model, about 20 s) and chunk
/// threads take them with `take_session` once they are ready. Each session runs single-threaded and all share one
/// prepacked copy of the weights.
///
/// With CoreML the session is opened for chunks of exactly `chunk_frames` frames: onnxruntime then folds the model's
/// shape arithmetic into constants before CoreML partitions the graph. Without that, CoreML takes the shape nodes too,
/// hands the CPU Slice nodes a 0-D `starts`, and the run fails ("Starts must be a 1-D array"). Chunks of any other
/// length are not this engine's to run (`runs`).
pub struct OrtBeat {
    path: PathBuf,
    chunk_frames: usize,
    env: Environment,
    prepacked: PrepackedWeights,
    ready: Mutex<Vec<OrtChunkSession>>,
    opened: AtomicUsize,
    all_open: AtomicBool,
    failure: OnceLock<String>,
}

pub struct OrtChunkSession {
    session: Session,
    input: String,
    beat_out: String,
    down_out: String,
}

const ACCELERATED: bool = cfg!(all(feature = "coreml", target_os = "macos"));

pub const ENGINE: &str = if ACCELERATED { "coreml" } else { "ort-cpu" };

fn pick(names: &[String], wanted: &[&str]) -> Result<String> {
    wanted
        .iter()
        .find(|w| names.iter().any(|n| n == *w))
        .map(|w| w.to_string())
        .ok_or_else(|| anyhow!("model has none of the nodes {wanted:?} (has {names:?})"))
}

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow!("onnxruntime: {e}")
}

fn cpu() -> ort::ep::CPU {
    ort::ep::CPU::default().with_arena_allocator(true)
}

#[cfg(all(feature = "coreml", target_os = "macos"))]
fn coreml_cache_dir(model: &Path, chunk_frames: usize) -> Result<PathBuf> {
    use std::hash::Hasher;
    use std::io::Read;
    let mut file = std::fs::File::open(model)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    let root = PathBuf::from(std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?).join("Library/Caches/fourfour-coreml");
    let key = format!("{:016x}-{chunk_frames}", hasher.finish());
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten().filter(|e| e.file_name().to_str() != Some(key.as_str())) {
            std::fs::remove_dir_all(entry.path()).ok();
        }
    }
    let dir = root.join(key);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

impl OrtBeat {
    pub fn new(path: &Path, chunk_frames: usize) -> Result<Self> {
        static ENV: OnceLock<std::result::Result<Environment, String>> = OnceLock::new();
        let env = ENV.get_or_init(|| ort::init().with_name("fourfour-beat").build().map_err(|e| e.to_string())).clone().map_err(ort_err)?;
        Ok(OrtBeat {
            path: path.to_path_buf(),
            chunk_frames,
            env,
            prepacked: PrepackedWeights::new(),
            ready: Mutex::new(Vec::new()),
            opened: AtomicUsize::new(0),
            all_open: AtomicBool::new(false),
            failure: OnceLock::new(),
        })
    }

    /// Whether this engine runs a chunk of this shape. A CoreML session takes only full-length chunks.
    pub fn runs(&self, chunk: &Tensor) -> bool {
        !ACCELERATED || chunk.shape.get(1) == Some(&self.chunk_frames)
    }

    pub fn failed(&self) -> Option<&str> {
        self.failure.get().map(String::as_str)
    }

    pub fn fail(&self, reason: String) {
        if self.failure.set(reason).is_ok() {
            eprintln!("beat model: {ENGINE} unavailable, using rten: {}", self.failure.get().map_or("", String::as_str));
        }
        self.ready.lock().unwrap().clear();
    }

    pub fn take_session(&self) -> Option<OrtChunkSession> {
        self.ready.lock().unwrap().pop()
    }

    /// Opens the first session alone, so a model compiles into the cache once, then the others side by side.
    pub fn open_sessions(&self, count: usize) {
        let open_one = || match self.open_session() {
            Ok(session) => {
                self.ready.lock().unwrap().push(session);
                self.opened.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => self.fail(format!("{e:#}")),
        };
        open_one();
        std::thread::scope(|s| {
            for _ in 1..count {
                if self.failed().is_none() {
                    s.spawn(open_one);
                }
            }
        });
        self.all_open.store(self.failed().is_none(), Ordering::Relaxed);
    }

    pub fn opened(&self) -> usize {
        self.opened.load(Ordering::Relaxed)
    }

    pub fn all_open(&self) -> bool {
        self.all_open.load(Ordering::Relaxed)
    }

    fn open_session(&self) -> Result<OrtChunkSession> {
        #[cfg(all(feature = "coreml", target_os = "macos"))]
        {
            let dir = coreml_cache_dir(&self.path, self.chunk_frames)?;
            match self.open_with(Some(&dir)) {
                Ok(session) => Ok(session),
                Err(first) => {
                    std::fs::remove_dir_all(&dir).ok();
                    std::fs::create_dir_all(&dir)?;
                    self.open_with(Some(&dir)).map_err(|second| anyhow!("{second:#} (and before clearing the compiled-model cache: {first:#})"))
                }
            }
        }
        #[cfg(not(all(feature = "coreml", target_os = "macos")))]
        self.open_with(None)
    }

    fn open_with(&self, cache: Option<&Path>) -> Result<OrtChunkSession> {
        let open = || -> ort::Result<Session> {
            let builder = Session::builder(&self.env)?
                .with_optimization_level(GraphOptimizationLevel::All)?
                .with_intra_threads(1)?
                .with_inter_threads(1)?
                .with_parallel_execution(false)?
                .with_intra_op_spinning(false)?
                .with_memory_pattern(false)?
                .with_prepacked_weights(&self.prepacked)?;
            self.execution_providers(builder, cache)?.commit_from_file(&self.path)
        };
        let session = open().map_err(|e| anyhow!("onnxruntime: loading {}: {e}", self.path.display()))?;
        let inputs: Vec<String> = session.inputs().iter().map(|o| o.name().to_string()).collect();
        let outputs: Vec<String> = session.outputs().iter().map(|o| o.name().to_string()).collect();
        Ok(OrtChunkSession {
            input: pick(&inputs, &["spectrogram"])?,
            beat_out: pick(&outputs, &["beat", "beat_logits"])?,
            down_out: pick(&outputs, &["downbeat", "downbeat_logits"])?,
            session,
        })
    }

    fn execution_providers(&self, builder: SessionBuilder, cache: Option<&Path>) -> ort::Result<SessionBuilder> {
        #[cfg(all(feature = "coreml", target_os = "macos"))]
        {
            use ort::ep::coreml::{ComputeUnits, ModelFormat};
            let mut coreml = ort::ep::CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::All);
            if let Some(dir) = cache {
                coreml = coreml.with_model_cache_dir(dir.display());
            }
            let builder = builder.with_dimension_override("batch", 1)?.with_dimension_override("time", self.chunk_frames as i64)?;
            return Ok(builder.with_execution_providers([coreml.build(), cpu().build()])?);
        }
        #[allow(unreachable_code)]
        {
            let _ = cache;
            Ok(builder.with_execution_providers([cpu().build()])?)
        }
    }
}

impl OrtChunkSession {
    pub fn run(&mut self, chunk: &Tensor) -> Result<(Vec<f32>, Vec<f32>)> {
        let input = TensorRef::from_array_view((chunk.shape.as_slice(), chunk.data.as_slice())).map_err(ort_err)?;
        let outs = self.session.run(ort::inputs![self.input.as_str() => input]).map_err(ort_err)?;
        let take = |name: &str| -> Result<Vec<f32>> {
            let (_, data) = outs[name].try_extract_tensor::<f32>().map_err(ort_err)?;
            Ok(data.to_vec())
        };
        Ok((take(&self.beat_out)?, take(&self.down_out)?))
    }
}
