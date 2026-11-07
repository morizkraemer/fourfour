use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rten::{ModelOptions, RunOptions, ThreadPool, ValueOrView, ValueView};

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn process_cpu_ms() -> (f64, f64) {
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = text.rsplit(')').next().unwrap_or("").split_whitespace().collect();
    let n = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (n(11) * 10.0, n(12) * 10.0)
}

fn fnv(values: &[f32]) -> u64 {
    values.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, v| (h ^ v.to_bits() as u64).wrapping_mul(0x100_0000_01b3))
}

fn main() -> anyhow::Result<()> {
    let dir = std::env::var_os("FOURFOUR_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/fourfour-models"));
    let file: String = env("MODEL_FILE", "beat_this.onnx".to_string());
    let frames: usize = env("CHUNK_FRAMES", 500);
    let rounds: usize = env("CHUNK_ROUNDS", 5);
    let threads: usize = env("POOL_THREADS", 1);
    let prepack: bool = env("PREPACK", 0u8) == 1;
    let timing: bool = env("TIMING", 1u8) == 1;

    let loaded = Instant::now();
    let mut options = ModelOptions::with_all_ops();
    options.prepack_weights(prepack);
    let model = options.load_file(dir.join(&file))?;
    eprintln!("load {file} prepack={prepack}: {:.0} ms", loaded.elapsed().as_secs_f64() * 1000.0);
    let input = model.find_node("spectrogram").unwrap();
    let beat = model.find_node("beat").or_else(|| model.find_node("beat_logits")).unwrap();
    let down = model.find_node("downbeat").or_else(|| model.find_node("downbeat_logits")).unwrap();

    let mut seed = 0x2545_f491_u32;
    let data: Vec<f32> = (0..frames * 128)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 1000) as f32 / 250.0
        })
        .collect();
    let pool = Arc::new(ThreadPool::with_num_threads(threads));
    let mut cpu = Vec::new();
    let mut wall = Vec::new();
    for round in 0..rounds {
        let last = round + 1 == rounds;
        let opts = RunOptions::default().with_thread_pool(Some(pool.clone())).with_timing(timing && last).with_timing_by_shape(env("BY_SHAPE", 0u8) == 1);
        let view = ValueView::from_shape([1, frames, 128].as_slice(), data.as_slice()).map_err(|e| anyhow::anyhow!("{e}"))?;
        let (u0, s0) = process_cpu_ms();
        let t = Instant::now();
        let outs = model.run(vec![(input, ValueOrView::from(view))], &[beat, down], Some(opts))?;
        wall.push(t.elapsed().as_secs_f64() * 1000.0);
        let (u1, s1) = process_cpu_ms();
        cpu.push((u1 - u0, s1 - s0));
        if last {
            let mut it = outs.into_iter();
            let b = it.next().unwrap().into_tensor::<f32>().unwrap().into_data();
            let d = it.next().unwrap().into_tensor::<f32>().unwrap().into_data();
            println!("LOGITS fnv beat {:016x} down {:016x}", fnv(&b), fnv(&d));
        }
    }
    let mut sorted = wall.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "RUN {file} frames={frames} threads={threads} prepack={prepack}: wall ms median {:.0} min {:.0}; user/sys ms per run {:?}",
        sorted[sorted.len() / 2],
        sorted[0],
        cpu.iter().map(|(u, s)| (u.round() as i64, s.round() as i64)).collect::<Vec<_>>()
    );
    Ok(())
}
