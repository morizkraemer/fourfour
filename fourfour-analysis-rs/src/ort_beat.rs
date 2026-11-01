use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use beat_this::Tensor;
use ort::environment::Environment;
use ort::session::builder::{GraphOptimizationLevel, PrepackedWeights, SessionBuilder};
use ort::session::Session;
use ort::value::TensorRef;

/// The beat model on onnxruntime. `run` takes the session mutably, so every chunk thread opens its own session; they
/// share one prepacked copy of the weights. Each session runs single-threaded, like the rten chunk pools.
///
/// With CoreML the session is opened for chunks of exactly `chunk_frames` frames: onnxruntime then folds the model's
/// shape arithmetic into constants before CoreML partitions the graph. Without that, CoreML takes the shape nodes too,
/// hands the CPU Slice nodes a 0-D `starts`, and the run fails ("Starts must be a 1-D array"). Shorter chunks (tracks
/// under one chunk long) go to a CPU session opened on first use. The compiled CoreML model is cached per model file.
pub struct OrtBeat {
    path: PathBuf,
    chunk_frames: usize,
    env: Environment,
    prepacked: PrepackedWeights,
    input: String,
    beat_out: String,
    down_out: String,
}

pub struct OrtChunkSession<'a> {
    beat: &'a OrtBeat,
    session: Session,
    other_lengths: Option<Session>,
}

#[derive(Clone, Copy, PartialEq)]
enum Device {
    Accelerated,
    Cpu,
}

const ACCELERATED: bool = cfg!(all(feature = "coreml", target_os = "macos"));

impl Default for Device {
    fn default() -> Self {
        if ACCELERATED { Device::Accelerated } else { Device::Cpu }
    }
}

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
fn coreml_cache_dir(model: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(model).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let dir = PathBuf::from(std::env::var_os("HOME")?)
        .join("Library/Caches/fourfour-coreml")
        .join(format!("{}-{modified}", meta.len()));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn execution_providers(builder: SessionBuilder, device: Device, model: &Path, chunk_frames: usize) -> ort::Result<SessionBuilder> {
    #[cfg(all(feature = "coreml", target_os = "macos"))]
    if device == Device::Accelerated {
        use ort::ep::coreml::{ComputeUnits, ModelFormat};
        let mut coreml = ort::ep::CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::All);
        if let Some(dir) = coreml_cache_dir(model) {
            coreml = coreml.with_model_cache_dir(dir.display());
        }
        let builder = builder.with_dimension_override("batch", 1)?.with_dimension_override("time", chunk_frames as i64)?;
        return Ok(builder.with_execution_providers([coreml.build(), cpu().build()])?);
    }
    let _ = (device, model, chunk_frames);
    Ok(builder.with_execution_providers([cpu().build()])?)
}

impl OrtBeat {
    pub fn load(path: &Path, chunk_frames: usize) -> Result<Self> {
        static ENV: OnceLock<std::result::Result<Environment, String>> = OnceLock::new();
        let env = ENV.get_or_init(|| ort::init().with_name("fourfour-beat").build().map_err(|e| e.to_string())).clone().map_err(ort_err)?;
        let mut beat = OrtBeat {
            path: path.to_path_buf(),
            chunk_frames,
            env,
            prepacked: PrepackedWeights::new(),
            input: String::new(),
            beat_out: String::new(),
            down_out: String::new(),
        };
        let session = beat.open_session(Device::default())?;
        let inputs: Vec<String> = session.inputs().iter().map(|o| o.name().to_string()).collect();
        let outputs: Vec<String> = session.outputs().iter().map(|o| o.name().to_string()).collect();
        beat.input = pick(&inputs, &["spectrogram"])?;
        beat.beat_out = pick(&outputs, &["beat", "beat_logits"])?;
        beat.down_out = pick(&outputs, &["downbeat", "downbeat_logits"])?;
        Ok(beat)
    }

    fn open_session(&self, device: Device) -> Result<Session> {
        let open = || -> ort::Result<Session> {
            let builder = Session::builder(&self.env)?
                .with_optimization_level(GraphOptimizationLevel::All)?
                .with_intra_threads(1)?
                .with_inter_threads(1)?
                .with_parallel_execution(false)?
                .with_intra_op_spinning(false)?
                .with_memory_pattern(false)?
                .with_prepacked_weights(&self.prepacked)?;
            Ok(execution_providers(builder, device, &self.path, self.chunk_frames)?.commit_from_file(&self.path)?)
        };
        open().map_err(|e| anyhow!("onnxruntime: loading {}: {e}", self.path.display()))
    }

    pub fn session(&self) -> Result<OrtChunkSession<'_>> {
        Ok(OrtChunkSession { beat: self, session: self.open_session(Device::default())?, other_lengths: None })
    }
}

impl OrtChunkSession<'_> {
    pub fn run(&mut self, chunk: &Tensor) -> Result<(Vec<f32>, Vec<f32>)> {
        let input = TensorRef::from_array_view((chunk.shape.as_slice(), chunk.data.as_slice())).map_err(ort_err)?;
        let session = if ACCELERATED && chunk.shape.get(1) != Some(&self.beat.chunk_frames) {
            match &mut self.other_lengths {
                Some(session) => session,
                none => none.insert(self.beat.open_session(Device::Cpu)?),
            }
        } else {
            &mut self.session
        };
        let outs = session.run(ort::inputs![self.beat.input.as_str() => input]).map_err(ort_err)?;
        let take = |name: &str| -> Result<Vec<f32>> {
            let (_, data) = outs[name].try_extract_tensor::<f32>().map_err(ort_err)?;
            Ok(data.to_vec())
        };
        Ok((take(&self.beat.beat_out)?, take(&self.beat.down_out)?))
    }
}
