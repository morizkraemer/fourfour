// Chunk scheduling and peak picking follow beat-this-rs (MIT, see NOTICE).

use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use beat_this::__probe::MelStream;

use crate::analyze::{Cancelled, Control};
use crate::decode::resample_to;
use beat_this::{Model as BtModel, Tensor};
use rten::{ModelOptions, NodeId, RunOptions, ThreadPool, ValueOrView, ValueView};

const COREML_CHUNK_SIZE: usize = 1500;
const RTEN_CHUNK_SIZE: usize = 500;
const DEFAULT_CHUNK_SIZE: usize = if cfg!(all(feature = "coreml", target_os = "macos")) { COREML_CHUNK_SIZE } else { RTEN_CHUNK_SIZE };
const DEFAULT_BORDER_SIZE: usize = 6;
const RTEN_OVERLAP: usize = 0;
const MEL_STRIDE: usize = 1536;
const N_MELS: usize = 128;
const FPS: f32 = 50.0;
pub const MODEL_RATE: u32 = 22050;
const PAUSE_POLL: Duration = Duration::from_millis(50);
const SILENT_CHUNK_LOGIT: f32 = -1000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Chunking {
    size: usize,
    border: usize,
    overlap: usize,
    blend: bool,
}

impl Chunking {
    fn from_env() -> Self {
        let env_frames = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<usize>().ok());
        let size = env_frames("FOURFOUR_BEAT_CHUNK").filter(|&n| n > 4 * DEFAULT_BORDER_SIZE).unwrap_or(DEFAULT_CHUNK_SIZE);
        let border = env_frames("FOURFOUR_BEAT_BORDER").filter(|&n| 4 * n < size).unwrap_or(DEFAULT_BORDER_SIZE);
        let default_overlap = if size == RTEN_CHUNK_SIZE { RTEN_OVERLAP } else { 0 };
        let overlap = env_frames("FOURFOUR_BEAT_OVERLAP").unwrap_or(default_overlap).min(size - 2 * border - 1);
        let blend = std::env::var("FOURFOUR_BEAT_BLEND").map_or(overlap > 0, |v| v == "1");
        Chunking { size, border, overlap, blend }
    }

    fn stride(&self) -> usize {
        self.size - 2 * self.border - self.overlap
    }
}

fn chunking() -> &'static Chunking {
    static CHUNKING: OnceLock<Chunking> = OnceLock::new();
    CHUNKING.get_or_init(Chunking::from_env)
}

fn chunk_size() -> usize {
    chunking().size
}

fn skip_silence() -> bool {
    static SKIP: OnceLock<bool> = OnceLock::new();
    *SKIP.get_or_init(|| std::env::var("FOURFOUR_BEAT_SKIP_SILENCE").is_ok_and(|v| v == "1"))
}

pub struct BeatEngine {
    mel: Arc<rten::Model>,
    mel_in: NodeId,
    mel_out: NodeId,
    beat: Arc<BeatModel>,
    chunks: Arc<ChunkExecutor>,
    chunk_threads: Mutex<Vec<JoinHandle<()>>>,
    shared_chunk_threads: AtomicUsize,
}

struct RtenBeat {
    model: rten::Model,
    input: NodeId,
    beat_out: NodeId,
    down_out: NodeId,
}

struct BeatModel {
    rten: RtenBeat,
    #[cfg(feature = "ort")]
    ort: Option<crate::ort_beat::OrtBeat>,
    chunks_accelerated: AtomicUsize,
    chunks_rten: AtomicUsize,
    chunks_silent: AtomicUsize,
}

struct ChunkRunner<'a> {
    model: &'a BeatModel,
    options: RunOptions,
    #[cfg(feature = "ort")]
    ort: Option<crate::ort_beat::OrtChunkSession>,
}

/// Beat-model chunk threads while onnxruntime serves the model. The Mac bench (R24, M5 Pro) had 2 CoreML sessions
/// at 26-31 tracks/min, 1 at 18-21; each session holds its own compiled model and activations, so more cost memory.
#[cfg(feature = "ort")]
const ORT_SESSIONS: usize = 2;

#[cfg(feature = "ort")]
const RTEN_CHUNK_SLOTS_PER_OPEN_SESSION: usize = 6;

/// The runtime the beat model is built to run on: `rten`, or onnxruntime (`coreml` on macOS with the `coreml`
/// feature, `ort-cpu` otherwise) when built with the `ort` feature and `FOURFOUR_BEAT_RUNTIME` is not `rten`.
pub fn beat_runtime() -> &'static str {
    #[cfg(feature = "ort")]
    if std::env::var("FOURFOUR_BEAT_RUNTIME").map_or(true, |v| v != "rten") {
        return crate::ort_beat::ENGINE;
    }
    "rten"
}

type ChunkLogits = (Vec<f32>, Vec<f32>);

/// Runs the beat model one chunk at a time on each of a set of threads that every track of the engine shares. A
/// track whose chunks are all taken leaves its threads to the others, so a batch never ends with one long track on
/// one thread while the other cores wait. Every chunk runs on a single-thread rten pool, the setup whose logits
/// match the reference bit for bit; tracks are served in the order they arrived.
#[derive(Default)]
struct ChunkExecutor {
    queue: Mutex<ChunkQueue>,
    ready: Condvar,
    wanted: AtomicUsize,
    running: AtomicUsize,
}

#[derive(Default)]
struct ChunkQueue {
    tracks: VecDeque<Arc<TrackChunks>>,
    shutdown: bool,
}

struct TrackChunks {
    mel: Tensor,
    starts: Vec<i32>,
    next: AtomicUsize,
    paused: AtomicBool,
    cancelled: AtomicBool,
    results: Mutex<Vec<Option<Result<ChunkLogits>>>>,
    finished: Condvar,
}

impl ChunkExecutor {
    fn take(&self, runner: &mut ChunkRunner) -> Option<(Arc<TrackChunks>, usize)> {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if queue.shutdown || runner.surplus(self.running.load(Ordering::Relaxed), self.wanted.load(Ordering::Relaxed)) {
                self.running.fetch_sub(1, Ordering::Relaxed);
                return None;
            }
            queue.tracks.retain(|t| !t.cancelled.load(Ordering::Relaxed));
            if let Some(pos) = queue.tracks.iter().position(|t| !t.paused.load(Ordering::Relaxed)) {
                let track = queue.tracks[pos].clone();
                let i = track.next.fetch_add(1, Ordering::Relaxed);
                if i + 1 >= track.starts.len() {
                    queue.tracks.remove(pos);
                }
                return Some((track, i));
            }
            queue = self.ready.wait_timeout(queue, PAUSE_POLL).unwrap().0;
        }
    }

    fn work(&self, model: &BeatModel) {
        let mut runner = ChunkRunner {
            model,
            options: RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1)))),
            #[cfg(feature = "ort")]
            ort: None,
        };
        while let Some((track, i)) = self.take(&mut runner) {
            let chunk = extract_chunk(&track.mel, track.starts[i], chunking());
            if skip_silence() && chunk.data.iter().all(|&v| v == 0.0) {
                let frames = chunk.shape[1];
                model.chunks_silent.fetch_add(1, Ordering::Relaxed);
                track.results.lock().unwrap()[i] = Some(Ok((vec![SILENT_CHUNK_LOGIT; frames], vec![SILENT_CHUNK_LOGIT; frames])));
                track.finished.notify_all();
                continue;
            }
            let logits = catch_unwind(AssertUnwindSafe(|| runner.run(&chunk))).unwrap_or_else(|_| Err(anyhow!("beat model panicked on chunk {i}")));
            track.results.lock().unwrap()[i] = Some(logits);
            track.finished.notify_all();
        }
    }
}

impl BeatModel {
    #[cfg(feature = "ort")]
    fn usable_ort(&self) -> Option<&crate::ort_beat::OrtBeat> {
        self.ort.as_ref().filter(|ort| ort.failed().is_none())
    }

    fn max_chunk_threads(&self, wanted: usize) -> usize {
        #[cfg(feature = "ort")]
        if let Some(ort) = self.usable_ort() {
            if ort.all_open() {
                return ORT_SESSIONS;
            }
            let open = ort.opened();
            return open + wanted.saturating_sub(open * RTEN_CHUNK_SLOTS_PER_OPEN_SESSION).max(1);
        }
        wanted
    }
}

impl ChunkRunner<'_> {
    /// True for a thread without a session while more threads run than the open sessions leave room for: it ran rten
    /// chunks while they opened and now leaves the memory to the sessions.
    #[cfg_attr(not(feature = "ort"), allow(unused_variables))]
    fn surplus(&mut self, running: usize, wanted: usize) -> bool {
        #[cfg(feature = "ort")]
        if let Some(ort) = self.model.usable_ort() {
            if self.ort.is_none() {
                self.ort = ort.take_session();
            }
            return self.ort.is_none() && running > self.model.max_chunk_threads(wanted);
        }
        false
    }

    /// Runs a chunk on onnxruntime once this thread has a session, on rten until then (the first CoreML sessions take
    /// seconds to open), for chunks onnxruntime does not take, and for good once onnxruntime failed.
    fn run(&mut self, chunk: &Tensor) -> Result<ChunkLogits> {
        #[cfg(feature = "ort")]
        if let Some(ort) = self.model.usable_ort() {
            if self.ort.is_none() {
                self.ort = ort.take_session();
            }
            if let Some(session) = self.ort.as_mut().filter(|_| ort.runs(chunk)) {
                match session.run(chunk) {
                    Ok(logits) => {
                        self.model.chunks_accelerated.fetch_add(1, Ordering::Relaxed);
                        return Ok(logits);
                    }
                    Err(e) => {
                        ort.fail(format!("running a chunk: {e:#}"));
                        self.ort = None;
                    }
                }
            }
        } else {
            self.ort = None;
        }
        self.model.chunks_rten.fetch_add(1, Ordering::Relaxed);
        self.model.rten.run_chunk(chunk, &self.options)
    }
}

impl RtenBeat {
    fn run_chunk(&self, chunk: &Tensor, options: &RunOptions) -> Result<ChunkLogits> {
        let view = ValueView::from_shape(chunk.shape.as_slice(), chunk.data.as_slice()).map_err(|e| anyhow!("beat input: {e}"))?;
        let outs = self.model.run(vec![(self.input, ValueOrView::from(view))], &[self.beat_out, self.down_out], Some(options.clone()))?;
        let mut it = outs.into_iter();
        let b = it.next().unwrap().into_tensor::<f32>().ok_or_else(|| anyhow!("beat output is not f32"))?;
        let d = it.next().unwrap().into_tensor::<f32>().ok_or_else(|| anyhow!("downbeat output is not f32"))?;
        Ok((b.into_data(), d.into_data()))
    }
}

impl Drop for BeatEngine {
    fn drop(&mut self) {
        self.chunks.queue.lock().unwrap().shutdown = true;
        self.chunks.ready.notify_all();
        for handle in self.chunk_threads.get_mut().unwrap().drain(..) {
            handle.join().ok();
        }
    }
}

#[derive(Clone, Debug)]
pub struct BeatStatus {
    pub runtime: &'static str,
    pub fallback: Option<String>,
    pub chunks_accelerated: usize,
    pub chunks_rten: usize,
    pub chunks_silent: usize,
}

pub struct BeatResult {
    pub beat_logits: Vec<f32>,
    pub down_logits: Vec<f32>,
    pub beats: Vec<f32>,
    pub downbeats: Vec<f32>,
    pub resample_ms: f64,
    pub mel_ms: f64,
    pub predict_ms: f64,
}

struct MelRunner<'a> {
    engine: &'a BeatEngine,
    options: RunOptions,
}

impl BtModel for MelRunner<'_> {
    fn run(&mut self, inputs: &[(&str, &Tensor)]) -> Result<HashMap<String, Tensor>> {
        let (_, t) = inputs[0];
        let view = ValueView::from_shape(t.shape.as_slice(), t.data.as_slice()).map_err(|e| anyhow!("mel input: {e}"))?;
        let outs = self.engine.mel.run(vec![(self.engine.mel_in, ValueOrView::from(view))], &[self.engine.mel_out], Some(self.options.clone()))?;
        let tensor = outs.into_iter().next().unwrap().into_tensor::<f32>().ok_or_else(|| anyhow!("mel output is not f32"))?;
        let shape = rten_tensor::Layout::shape(&tensor).to_vec();
        let mut m = HashMap::new();
        m.insert("mel_spectrogram".to_string(), Tensor { shape, data: tensor.into_data() });
        Ok(m)
    }
}

fn load(path: &Path, prepack: bool) -> Result<rten::Model> {
    let mut o = ModelOptions::with_all_ops();
    o.prepack_weights(prepack);
    Ok(o.load_file(path)?)
}

impl BeatEngine {
    pub fn load(models_dir: &Path, beat_file: &str, prepack: bool) -> Result<Self> {
        let mel = load(&models_dir.join("mel_spectrogram.onnx"), prepack)?;
        let find = |m: &rten::Model, names: &[&str]| -> Result<NodeId> {
            names.iter().find_map(|n| m.find_node(n)).ok_or_else(|| anyhow!("model has none of the nodes {names:?}"))
        };
        let beat_path = models_dir.join(beat_file);
        let rten = load(&beat_path, prepack)?;
        let beat = Arc::new(BeatModel {
            rten: RtenBeat {
                input: find(&rten, &["spectrogram"])?,
                beat_out: find(&rten, &["beat", "beat_logits"])?,
                down_out: find(&rten, &["downbeat", "downbeat_logits"])?,
                model: rten,
            },
            #[cfg(feature = "ort")]
            ort: (beat_runtime() != "rten")
                .then(|| crate::ort_beat::OrtBeat::new(&beat_path, chunk_size()))
                .and_then(|ort| ort.inspect_err(|e| eprintln!("beat model: {} unavailable, using rten: {e:#}", crate::ort_beat::ENGINE)).ok()),
            chunks_accelerated: AtomicUsize::new(0),
            chunks_rten: AtomicUsize::new(0),
            chunks_silent: AtomicUsize::new(0),
        });
        #[cfg(feature = "ort")]
        if beat.ort.is_some() {
            let opener = beat.clone();
            std::thread::Builder::new()
                .name("beat-sessions".into())
                .spawn(move || opener.ort.as_ref().map(|ort| ort.open_sessions(ORT_SESSIONS)))?;
        }
        Ok(BeatEngine {
            mel_in: find(&mel, &["audio_pcm"])?,
            mel_out: find(&mel, &["mel_spectrogram"])?,
            beat,
            mel: Arc::new(mel),
            chunks: Arc::new(ChunkExecutor::default()),
            chunk_threads: Mutex::new(Vec::new()),
            shared_chunk_threads: AtomicUsize::new(0),
        })
    }

    /// The number of beat-model threads all tracks share, and so the number of chunks (each with its own activations)
    /// in memory at once; a track's own thread count no longer adds any. Without it every track brings its thread count.
    /// They start with the first track, so they inherit its priority (see `analysis_priority`).
    pub fn share_chunk_threads(&self, threads: usize) {
        self.shared_chunk_threads.store(threads.max(1), Ordering::Relaxed);
    }

    /// The runtime that runs the beat model now, the chunks each side ran so far and, after a fallback to rten, why.
    pub fn beat_status(&self) -> BeatStatus {
        #[cfg(feature = "ort")]
        let (runtime, fallback) = match &self.beat.ort {
            Some(ort) => match ort.failed() {
                Some(why) => ("rten", Some(format!("{} failed: {why}", crate::ort_beat::ENGINE))),
                None => (crate::ort_beat::ENGINE, None),
            },
            None => ("rten", (beat_runtime() != "rten").then(|| format!("{} failed to start", crate::ort_beat::ENGINE))),
        };
        #[cfg(not(feature = "ort"))]
        let (runtime, fallback) = ("rten", None);
        BeatStatus {
            runtime,
            fallback,
            chunks_accelerated: self.beat.chunks_accelerated.load(Ordering::Relaxed),
            chunks_rten: self.beat.chunks_rten.load(Ordering::Relaxed),
            chunks_silent: self.beat.chunks_silent.load(Ordering::Relaxed),
        }
    }

    fn ensure_chunk_threads(&self, wanted: usize) -> Result<()> {
        self.chunks.wanted.store(wanted, Ordering::Relaxed);
        let limit = self.beat.max_chunk_threads(wanted);
        let mut threads = self.chunk_threads.lock().unwrap();
        threads.retain(|thread| !thread.is_finished());
        while self.chunks.running.load(Ordering::Relaxed) < limit {
            let (chunks, model) = (self.chunks.clone(), self.beat.clone());
            self.chunks.running.fetch_add(1, Ordering::Relaxed);
            match std::thread::Builder::new().name("beat-chunks".into()).spawn(move || chunks.work(&model)) {
                Ok(handle) => threads.push(handle),
                Err(e) => {
                    self.chunks.running.fetch_sub(1, Ordering::Relaxed);
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    pub fn analyze(&self, samples: &[f32], source_rate: u32, threads: usize, control: &Control) -> Result<BeatResult> {
        self.analyze_resampled(|| resample_to(samples, source_rate, MODEL_RATE), threads, control)
    }

    /// `resample` returns the track at `MODEL_RATE`; the caller's source audio can be freed inside it, before the mel
    /// spectrogram and the beat-model chunks run.
    pub fn analyze_resampled(&self, resample: impl FnOnce() -> Result<Vec<f32>>, threads: usize, control: &Control) -> Result<BeatResult> {
        let t = Instant::now();
        let samples = resample()?;
        let resample_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let pool = Arc::new(ThreadPool::with_num_threads(threads.max(1)));
        let mut runner = MelRunner { engine: self, options: RunOptions::default().with_thread_pool(Some(pool)) };
        let mut stream = MelStream::new(MEL_STRIDE);
        stream.push(&mut runner, &samples)?;
        drop(samples);
        let mel = stream.finish(&mut runner)?;
        let mel_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let (beat_logits, down_logits) = self.predict(mel, threads, control)?;
        let predict_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (beats, downbeats) = pick_peaks(&beat_logits, &down_logits);
        Ok(BeatResult { beat_logits, down_logits, beats, downbeats, resample_ms, mel_ms, predict_ms })
    }

    fn predict(&self, mel: Tensor, threads: usize, control: &Control) -> Result<ChunkLogits> {
        let full_time = mel.shape[1];
        let starts = generate_starts(full_time, chunking());
        let shared = self.shared_chunk_threads.load(Ordering::Relaxed);
        self.ensure_chunk_threads(if shared > 0 { shared } else { threads.max(1) })?;
        let track = Arc::new(TrackChunks {
            results: Mutex::new(starts.iter().map(|_| None).collect()),
            mel,
            starts,
            next: AtomicUsize::new(0),
            paused: AtomicBool::new(control.pause.load(Ordering::Relaxed)),
            cancelled: AtomicBool::new(false),
            finished: Condvar::new(),
        });
        if !track.starts.is_empty() {
            self.chunks.queue.lock().unwrap().tracks.push_back(track.clone());
            self.chunks.ready.notify_all();
        }
        let mut results = track.results.lock().unwrap();
        while !results.iter().all(Option::is_some) {
            if control.cancel.load(Ordering::Relaxed) {
                track.cancelled.store(true, Ordering::Relaxed);
                return Err(Cancelled.into());
            }
            let paused = control.pause.load(Ordering::Relaxed);
            if track.paused.swap(paused, Ordering::Relaxed) && !paused {
                self.chunks.ready.notify_all();
            }
            results = track.finished.wait_timeout(results, PAUSE_POLL).unwrap().0;
        }
        if control.cancel.load(Ordering::Relaxed) {
            return Err(Cancelled.into());
        }
        let mut chunks = Vec::with_capacity(track.starts.len());
        for (i, &start) in track.starts.iter().enumerate() {
            chunks.push((start, results[i].take().unwrap()?));
        }
        Ok(stitch(&chunks, full_time, chunking()))
    }
}

fn stitch(chunks: &[(i32, ChunkLogits)], full_time: usize, plan: &Chunking) -> ChunkLogits {
    let border = plan.border;
    if plan.blend {
        let mut beat_logits = vec![0.0f32; full_time];
        let mut down_logits = vec![0.0f32; full_time];
        let mut weight_sum = vec![0.0f32; full_time];
        let ramp = (plan.overlap + 1) as f32;
        for (start, (beat, down)) in chunks {
            let chunk_time = beat.len();
            for j in border..chunk_time - border {
                let dest = (*start + j as i32) as usize;
                if dest >= full_time {
                    continue;
                }
                let weight = ((j - border + 1) as f32 / ramp).min((chunk_time - border - j) as f32 / ramp).min(1.0);
                beat_logits[dest] += weight * beat[j];
                down_logits[dest] += weight * down[j];
                weight_sum[dest] += weight;
            }
        }
        for i in 0..full_time {
            if weight_sum[i] > 0.0 {
                beat_logits[i] /= weight_sum[i];
                down_logits[i] /= weight_sum[i];
            } else {
                beat_logits[i] = SILENT_CHUNK_LOGIT;
                down_logits[i] = SILENT_CHUNK_LOGIT;
            }
        }
        return (beat_logits, down_logits);
    }
    let mut beat_logits = vec![SILENT_CHUNK_LOGIT; full_time];
    let mut down_logits = vec![SILENT_CHUNK_LOGIT; full_time];
    for (start, (beat, down)) in chunks.iter().rev() {
        let chunk_time = beat.len();
        let write_start = (*start + border as i32) as usize;
        for (k, j) in (border..chunk_time - border).enumerate() {
            let dest = write_start + k;
            if dest < full_time {
                beat_logits[dest] = beat[j];
                down_logits[dest] = down[j];
            }
        }
    }
    (beat_logits, down_logits)
}

fn generate_starts(full_time: usize, plan: &Chunking) -> Vec<i32> {
    let mut starts = Vec::new();
    let mut pos = -(plan.border as i32);
    let limit = full_time as i32 - plan.border as i32;
    while pos < limit {
        starts.push(pos);
        pos += plan.stride() as i32;
    }
    if full_time > plan.stride() {
        if let Some(last) = starts.last_mut() {
            *last = (full_time as i32 - (plan.size as i32 - plan.border as i32)).max(-(plan.border as i32));
        }
    }
    starts
}

fn extract_chunk(mel: &Tensor, start: i32, plan: &Chunking) -> Tensor {
    let (size, border) = (plan.size, plan.border);
    let full_time = mel.shape[1];
    let actual_start = start.max(0) as usize;
    let actual_end = ((start + size as i32) as usize).min(full_time);
    let pad_left = (-start).max(0) as usize;
    let n_frames = actual_end - actual_start;
    let pad_right = 0.max((start + size as i32 - full_time as i32).min(border as i32)) as usize;
    let chunk_time = pad_left + n_frames + pad_right;
    let mut data = vec![0.0f32; chunk_time * N_MELS];
    data[pad_left * N_MELS..(pad_left + n_frames) * N_MELS].copy_from_slice(&mel.data[actual_start * N_MELS..actual_end * N_MELS]);
    Tensor { shape: vec![1, chunk_time, N_MELS], data }
}

fn find_peaks(logits: &[f32]) -> Vec<f64> {
    let len = logits.len();
    let mut peaks = Vec::new();
    for i in 0..len {
        if logits[i] <= 0.0 {
            continue;
        }
        let (start, end) = (i.saturating_sub(3), (i + 4).min(len));
        if logits[start..end].iter().all(|&v| v <= logits[i]) {
            peaks.push(i);
        }
    }
    dedupe(&peaks)
}

fn dedupe(peaks: &[usize]) -> Vec<f64> {
    if peaks.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut p = peaks[0] as f64;
    let mut c = 1.0_f64;
    for &p2 in &peaks[1..] {
        let p2 = p2 as f64;
        if p2 - p <= 1.0 {
            c += 1.0;
            p += (p2 - p) / c;
        } else {
            result.push(p);
            p = p2;
            c = 1.0;
        }
    }
    result.push(p);
    result
}

fn pick_peaks(beat_logits: &[f32], down_logits: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let beats: Vec<f32> = find_peaks(beat_logits).iter().map(|&f| (f / FPS as f64) as f32).collect();
    let mut downbeats: Vec<f32> = find_peaks(down_logits).iter().map(|&f| (f / FPS as f64) as f32).collect();
    if !beats.is_empty() && !downbeats.is_empty() {
        for d in downbeats.iter_mut() {
            let pos = beats.partition_point(|&b| b < *d);
            *d = match (pos.checked_sub(1), beats.get(pos)) {
                (Some(before), Some(&after)) => {
                    if (*d - beats[before]).abs() <= (after - *d).abs() { beats[before] } else { after }
                }
                (Some(before), None) => beats[before],
                (None, Some(&after)) => after,
                (None, None) => *d,
            };
        }
        downbeats.sort_by(|a, b| a.total_cmp(b));
        downbeats.dedup();
    }
    (beats, downbeats)
}

#[cfg(test)]
mod chunk_default_tests {
    use super::*;

    #[test]
    fn coreml_builds_keep_the_long_chunk_and_the_rest_use_the_short_one() {
        let coreml_on_mac = cfg!(all(feature = "coreml", target_os = "macos"));
        assert_eq!(DEFAULT_CHUNK_SIZE, if coreml_on_mac { 1500 } else { 500 });
    }

    fn position_logits(full_time: usize, plan: &Chunking) -> Vec<(i32, ChunkLogits)> {
        let mel = Tensor { shape: vec![1, full_time, N_MELS], data: vec![0.0; full_time * N_MELS] };
        generate_starts(full_time, plan)
            .into_iter()
            .map(|start| {
                let chunk_time = extract_chunk(&mel, start, plan).shape[1];
                let logits: Vec<f32> = (0..chunk_time).map(|j| (start + j as i32) as f32).collect();
                (start, (logits.clone(), logits))
            })
            .collect()
    }

    #[test]
    fn stitching_position_consistent_chunks_gives_the_positions_back() {
        let plans = [
            Chunking { size: 500, border: 6, overlap: 0, blend: false },
            Chunking { size: 500, border: 25, overlap: 0, blend: false },
            Chunking { size: 500, border: 6, overlap: 72, blend: false },
            Chunking { size: 500, border: 6, overlap: 72, blend: true },
            Chunking { size: 1500, border: 6, overlap: 0, blend: false },
        ];
        for plan in plans {
            for full_time in [100, 480, 488, 500, 1000, 4321, 13790] {
                let (beat, down) = stitch(&position_logits(full_time, &plan), full_time, &plan);
                for k in 0..full_time {
                    assert!((beat[k] - k as f32).abs() < 1e-2 && (down[k] - k as f32).abs() < 1e-2, "{plan:?} full_time {full_time} frame {k}: {} {}", beat[k], down[k]);
                }
            }
        }
    }

    #[test]
    fn blending_crossfades_two_chunks_across_the_overlap() {
        let plan = Chunking { size: 100, border: 5, overlap: 10, blend: true };
        let chunks = vec![(-5, (vec![0.0; 100], vec![0.0; 100])), (75, (vec![10.0; 100], vec![10.0; 100]))];
        let (beat, _) = stitch(&chunks, 150, &plan);
        assert_eq!(beat[10], 0.0);
        assert_eq!(beat[149], 10.0);
        let overlap: Vec<f32> = (80..90).map(|k| beat[k]).collect();
        assert!(overlap.windows(2).all(|w| w[0] < w[1]), "{overlap:?}");
        assert!(overlap[0] > 0.0 && overlap[9] < 10.0);
    }
}

#[cfg(all(test, feature = "ort"))]
mod ort_tests {
    use super::*;

    fn models_dir() -> std::path::PathBuf {
        std::env::var_os("FOURFOUR_MODELS_DIR")
            .map(Into::into)
            .unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/fourfour-models"))
    }

    #[test]
    #[ignore = "needs the beat model weights (FOURFOUR_MODELS_DIR)"]
    fn ort_gives_the_rten_logits_for_a_chunk() {
        let path = models_dir().join("beat_this.onnx");
        let model = load(&path, false).unwrap();
        let rten = RtenBeat {
            input: model.find_node("spectrogram").unwrap(),
            beat_out: model.find_node("beat").or_else(|| model.find_node("beat_logits")).unwrap(),
            down_out: model.find_node("downbeat").or_else(|| model.find_node("downbeat_logits")).unwrap(),
            model,
        };
        let ort = crate::ort_beat::OrtBeat::new(&path, DEFAULT_CHUNK_SIZE).unwrap();
        let open = || {
            ort.open_sessions(1);
            ort.take_session().unwrap_or_else(|| panic!("no session: {:?}", ort.failed()))
        };
        let mut seed = 0x2545_f491_u32;
        let data = (0..DEFAULT_CHUNK_SIZE * N_MELS)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed % 1000) as f32 / 250.0
            })
            .collect();
        let chunk = Tensor { shape: vec![1, DEFAULT_CHUNK_SIZE, N_MELS], data };
        let options = RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))));
        let (rb, rd) = rten.run_chunk(&chunk, &options).unwrap();
        let (ob, od) = open().run(&chunk).unwrap();
        assert_eq!((rb.len(), rd.len()), (ob.len(), od.len()));
        let max_diff = rb.iter().chain(&rd).zip(ob.iter().chain(&od)).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(max_diff < 1e-2, "max abs logit diff {max_diff}");

        let rounds: usize = std::env::var("FF_CHUNK_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let thread_cpu_ms = || -> f64 {
            let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
            let fields: Vec<f64> = stat.rsplit(')').next().unwrap_or("").split_whitespace().map(|v| v.parse().unwrap_or(0.0)).collect();
            (fields.get(11).unwrap_or(&0.0) + fields.get(12).unwrap_or(&0.0)) * 10.0
        };
        let wall_ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;
        let opened = std::time::Instant::now();
        let mut session = open();
        let open_ms = wall_ms(opened);
        let (mut rten_ms, mut ort_ms, mut rten_wall, mut ort_wall) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for _ in 0..rounds {
            let (t, w) = (thread_cpu_ms(), std::time::Instant::now());
            rten.run_chunk(&chunk, &options).unwrap();
            rten_ms.push(thread_cpu_ms() - t);
            rten_wall.push(wall_ms(w));
            let (t, w) = (thread_cpu_ms(), std::time::Instant::now());
            session.run(&chunk).unwrap();
            ort_ms.push(thread_cpu_ms() - t);
            ort_wall.push(wall_ms(w));
        }
        if rounds > 0 {
            let median = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            };
            println!(
                "CHUNK median of {rounds}: wall ms rten {:.0} ort {:.0}; process cpu ms rten {:.0} ort {:.0} (0 without /proc); session open {open_ms:.0} ms; max abs logit diff {max_diff}",
                median(&mut rten_wall),
                median(&mut ort_wall),
                median(&mut rten_ms),
                median(&mut ort_ms)
            );
        }
    }
}
