// Chunk scheduling and peak picking follow beat-this-rs (MIT, see NOTICE).

use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use beat_this::__probe::MelStream;

use crate::analyze::{Cancelled, Control};
use crate::decode::resample_to;
use beat_this::{Model as BtModel, Tensor};
use rten::{ModelOptions, NodeId, RunOptions, ThreadPool, ValueOrView, ValueView};

const CHUNK_SIZE: usize = 1500;
const BORDER_SIZE: usize = 6;
const STRIDE: usize = CHUNK_SIZE - 2 * BORDER_SIZE;
const MEL_STRIDE: usize = 1536;
const N_MELS: usize = 128;
const FPS: f32 = 50.0;
const PAUSE_POLL: Duration = Duration::from_millis(50);

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

enum BeatModel {
    Rten(RtenBeat),
    #[cfg(feature = "ort")]
    Ort(crate::ort_beat::OrtBeat),
}

enum ChunkRunner<'a> {
    Rten(&'a RtenBeat, RunOptions),
    #[cfg(feature = "ort")]
    Ort(crate::ort_beat::OrtChunkSession<'a>),
}

/// The runtime the beat model runs on: `rten`, or `ort` when built with the `ort` feature and
/// `FOURFOUR_BEAT_RUNTIME` is not `rten`.
pub fn beat_runtime() -> &'static str {
    if cfg!(feature = "ort") && std::env::var("FOURFOUR_BEAT_RUNTIME").map_or(true, |v| v != "rten") {
        "ort"
    } else {
        "rten"
    }
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
    fn take(&self) -> Option<(Arc<TrackChunks>, usize)> {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if queue.shutdown {
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
        let mut runner = model.runner();
        while let Some((track, i)) = self.take() {
            let chunk = extract_chunk(&track.mel, track.starts[i]);
            let logits = match runner.as_mut() {
                Ok(runner) => catch_unwind(AssertUnwindSafe(|| runner.run(&chunk)))
                    .unwrap_or_else(|_| Err(anyhow!("beat model panicked on chunk {i}"))),
                Err(e) => Err(anyhow!("beat model did not start: {e:#}")),
            };
            track.results.lock().unwrap()[i] = Some(logits);
            track.finished.notify_all();
        }
    }
}

impl BeatModel {
    fn runner(&self) -> Result<ChunkRunner<'_>> {
        match self {
            BeatModel::Rten(m) => {
                Ok(ChunkRunner::Rten(m, RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))))))
            }
            #[cfg(feature = "ort")]
            BeatModel::Ort(m) => Ok(ChunkRunner::Ort(m.session()?)),
        }
    }
}

impl ChunkRunner<'_> {
    fn run(&mut self, chunk: &Tensor) -> Result<ChunkLogits> {
        match self {
            ChunkRunner::Rten(m, options) => m.run_chunk(chunk, options),
            #[cfg(feature = "ort")]
            ChunkRunner::Ort(session) => session.run(chunk),
        }
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
        let beat = match beat_runtime() {
            #[cfg(feature = "ort")]
            "ort" => BeatModel::Ort(crate::ort_beat::OrtBeat::load(&beat_path, CHUNK_SIZE)?),
            _ => {
                let beat = load(&beat_path, prepack)?;
                BeatModel::Rten(RtenBeat {
                    input: find(&beat, &["spectrogram"])?,
                    beat_out: find(&beat, &["beat", "beat_logits"])?,
                    down_out: find(&beat, &["downbeat", "downbeat_logits"])?,
                    model: beat,
                })
            }
        };
        Ok(BeatEngine {
            mel_in: find(&mel, &["audio_pcm"])?,
            mel_out: find(&mel, &["mel_spectrogram"])?,
            beat: Arc::new(beat),
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

    fn ensure_chunk_threads(&self, wanted: usize) -> Result<()> {
        let mut threads = self.chunk_threads.lock().unwrap();
        while threads.len() < wanted {
            let (chunks, model) = (self.chunks.clone(), self.beat.clone());
            threads.push(std::thread::Builder::new().name("beat-chunks".into()).spawn(move || chunks.work(&model))?);
        }
        Ok(())
    }

    pub fn analyze(&self, samples: &[f32], source_rate: u32, threads: usize, control: &Control) -> Result<BeatResult> {
        let t = Instant::now();
        let samples = resample_to(samples, source_rate, 22050)?;
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
        let starts = generate_starts(full_time);
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
        let mut beat_logits = vec![-1000.0f32; full_time];
        let mut down_logits = vec![-1000.0f32; full_time];
        for (i, &start) in track.starts.iter().enumerate().rev() {
            let (beat, down) = results[i].take().unwrap()?;
            let chunk_time = beat.len();
            let valid = BORDER_SIZE..chunk_time - BORDER_SIZE;
            let write_start = (start + BORDER_SIZE as i32) as usize;
            for (k, j) in valid.enumerate() {
                let dest = write_start + k;
                if dest < full_time {
                    beat_logits[dest] = beat[j];
                    down_logits[dest] = down[j];
                }
            }
        }
        Ok((beat_logits, down_logits))
    }
}

fn generate_starts(full_time: usize) -> Vec<i32> {
    let mut starts = Vec::new();
    let mut pos = -(BORDER_SIZE as i32);
    let limit = full_time as i32 - BORDER_SIZE as i32;
    while pos < limit {
        starts.push(pos);
        pos += STRIDE as i32;
    }
    if full_time > STRIDE {
        if let Some(last) = starts.last_mut() {
            *last = full_time as i32 - (CHUNK_SIZE as i32 - BORDER_SIZE as i32);
        }
    }
    starts
}

fn extract_chunk(mel: &Tensor, start: i32) -> Tensor {
    let full_time = mel.shape[1];
    let actual_start = start.max(0) as usize;
    let actual_end = ((start + CHUNK_SIZE as i32) as usize).min(full_time);
    let pad_left = (-start).max(0) as usize;
    let n_frames = actual_end - actual_start;
    let pad_right = 0.max((start + CHUNK_SIZE as i32 - full_time as i32).min(BORDER_SIZE as i32)) as usize;
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
        let ort = crate::ort_beat::OrtBeat::load(&path, CHUNK_SIZE).unwrap();
        let mut seed = 0x2545_f491_u32;
        let data = (0..CHUNK_SIZE * N_MELS)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed % 1000) as f32 / 250.0
            })
            .collect();
        let chunk = Tensor { shape: vec![1, CHUNK_SIZE, N_MELS], data };
        let options = RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))));
        let (rb, rd) = rten.run_chunk(&chunk, &options).unwrap();
        let (ob, od) = ort.session().unwrap().run(&chunk).unwrap();
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
        let mut session = ort.session().unwrap();
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
