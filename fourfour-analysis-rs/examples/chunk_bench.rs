use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rten::{ModelOptions, RunOptions, ThreadPool, ValueOrView, ValueView};

const N_MELS: usize = 128;

fn process_cpu_ms() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<f64> = stat.rsplit(')').next().unwrap_or("").split_whitespace().map(|v| v.parse().unwrap_or(0.0)).collect();
    (fields.get(11).unwrap_or(&0.0) + fields.get(12).unwrap_or(&0.0)) * 10.0
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: chunk_bench <rounds> <frames,frames,...> <model.onnx>...";
    let rounds: usize = args.next().expect(usage).parse()?;
    let sizes: Vec<usize> = args.next().expect(usage).split(',').map(|s| s.parse().unwrap()).collect();
    let paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
    let models = paths
        .iter()
        .map(|p| {
            let m = ModelOptions::with_all_ops().load_file(p)?;
            let ids = (m.find_node("spectrogram").unwrap(), m.find_node("beat").unwrap(), m.find_node("downbeat").unwrap());
            Ok((p.file_name().unwrap().to_string_lossy().to_string(), m, ids))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let options = RunOptions::default().with_thread_pool(Some(Arc::new(ThreadPool::with_num_threads(1))));
    let mut seed = 0x2545_f491_u32;
    let max = *sizes.iter().max().unwrap();
    let data: Vec<f32> = (0..max * N_MELS)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 1000) as f32 / 250.0
        })
        .collect();
    let mut cpu: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); sizes.len()]; models.len()];
    let mut wall = cpu.clone();
    for round in 0..=rounds {
        for (mi, (_, model, (input, beat, down))) in models.iter().enumerate() {
            for (si, &frames) in sizes.iter().enumerate() {
                let view = ValueView::from_shape(&[1, frames, N_MELS][..], &data[..frames * N_MELS])?;
                let (c, w) = (process_cpu_ms(), Instant::now());
                model.run(vec![(*input, ValueOrView::from(view))], &[*beat, *down], Some(options.clone()))?;
                if round > 0 {
                    cpu[mi][si].push(process_cpu_ms() - c);
                    wall[mi][si].push(w.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    for (mi, (name, _, _)) in models.iter().enumerate() {
        for (si, &frames) in sizes.iter().enumerate() {
            let (c, w) = (median(&mut cpu[mi][si]), median(&mut wall[mi][si]));
            println!("{name} frames {frames}: cpu {c:.0} ms, wall {w:.0} ms, cpu per 1000 frames {:.0} ms", c * 1000.0 / frames as f64);
        }
    }
    Ok(())
}
