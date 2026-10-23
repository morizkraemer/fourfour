// Chunk scheduling and peak picking follow beat-this-rs (MIT, see NOTICE).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use beat_this::__probe::MelStream;

use crate::decode::resample_to;
use beat_this::{Model as BtModel, Tensor};
use rten::{ModelOptions, NodeId, RunOptions, ThreadPool, ValueOrView, ValueView};

const CHUNK_SIZE: usize = 1500;
const BORDER_SIZE: usize = 6;
const STRIDE: usize = CHUNK_SIZE - 2 * BORDER_SIZE;
const MEL_STRIDE: usize = 1536;
const N_MELS: usize = 128;
const FPS: f32 = 50.0;

pub struct BeatEngine {
    mel: Arc<rten::Model>,
    mel_in: NodeId,
    mel_out: NodeId,
    beat: Arc<rten::Model>,
    beat_in: NodeId,
    beat_out: NodeId,
    down_out: NodeId,
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
        let beat = load(&models_dir.join(beat_file), prepack)?;
        let find = |m: &rten::Model, names: &[&str]| -> Result<NodeId> {
            names.iter().find_map(|n| m.find_node(n)).ok_or_else(|| anyhow!("model has none of the nodes {names:?}"))
        };
        Ok(BeatEngine {
            mel_in: find(&mel, &["audio_pcm"])?,
            mel_out: find(&mel, &["mel_spectrogram"])?,
            beat_in: find(&beat, &["spectrogram"])?,
            beat_out: find(&beat, &["beat", "beat_logits"])?,
            down_out: find(&beat, &["downbeat", "downbeat_logits"])?,
            mel: Arc::new(mel),
            beat: Arc::new(beat),
        })
    }

    pub fn analyze(&self, samples: &[f32], source_rate: u32, threads: usize) -> Result<BeatResult> {
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
        let (beat_logits, down_logits) = self.predict(&mel, threads)?;
        let predict_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (beats, downbeats) = pick_peaks(&beat_logits, &down_logits);
        Ok(BeatResult { beat_logits, down_logits, beats, downbeats, resample_ms, mel_ms, predict_ms })
    }

    fn run_chunk(&self, chunk: &Tensor, options: &RunOptions) -> Result<(Vec<f32>, Vec<f32>)> {
        let view = ValueView::from_shape(chunk.shape.as_slice(), chunk.data.as_slice()).map_err(|e| anyhow!("beat input: {e}"))?;
        let outs = self.beat.run(vec![(self.beat_in, ValueOrView::from(view))], &[self.beat_out, self.down_out], Some(options.clone()))?;
        let mut it = outs.into_iter();
        let b = it.next().unwrap().into_tensor::<f32>().ok_or_else(|| anyhow!("beat output is not f32"))?;
        let d = it.next().unwrap().into_tensor::<f32>().ok_or_else(|| anyhow!("downbeat output is not f32"))?;
        Ok((b.into_data(), d.into_data()))
    }

    fn predict(&self, mel: &Tensor, threads: usize) -> Result<(Vec<f32>, Vec<f32>)> {
        let full_time = mel.shape[1];
        let starts = generate_starts(full_time);
        let workers = threads.max(1).min(starts.len());
        let next = AtomicUsize::new(0);
        let results: Vec<Mutex<Option<Result<(Vec<f32>, Vec<f32>)>>>> = starts.iter().map(|_| Mutex::new(None)).collect();
        let run_worker = |options: RunOptions| loop {
            let i = next.fetch_add(1, Ordering::SeqCst);
            if i >= starts.len() {
                break;
            }
            let chunk = extract_chunk(mel, starts[i]);
            *results[i].lock().unwrap() = Some(self.run_chunk(&chunk, &options));
        };
        if workers == 1 {
            run_worker(RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1)))));
        } else {
            std::thread::scope(|s| {
                for _ in 0..workers {
                    let run_worker = &run_worker;
                    s.spawn(move || run_worker(RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))))));
                }
            });
        }
        let mut beat_logits = vec![-1000.0f32; full_time];
        let mut down_logits = vec![-1000.0f32; full_time];
        for (i, &start) in starts.iter().enumerate().rev() {
            let (beat, down) = results[i].lock().unwrap().take().unwrap()?;
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
