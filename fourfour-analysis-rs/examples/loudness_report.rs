use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result};
use fourfour_analysis::analyze::{analyze_controlled, Config, Control};
use fourfour_analysis::bt::BeatEngine;
use fourfour_analysis::decode::{decode_mono, decode_mono_measured};
use fourfour_analysis::loudness::{default_channel_weights, Meter};
use fourfour_analysis::service::{locate_models, BEAT_MODEL};
use serde_json::json;

fn decode_only(path: &Path) -> Result<serde_json::Value> {
    let t = Instant::now();
    let mono = decode_mono_measured(path, true)?;
    let total_ms = t.elapsed().as_secs_f64() * 1000.0;
    let l = mono.loudness;
    Ok(json!({
        "path": path,
        "duration_s": mono.samples.len() as f64 / mono.rate as f64,
        "lufs": l.map(|l| l.lufs),
        "peak_db": l.map(|l| l.peak_db),
        "gain_db": l.map(|l| l.gain_db),
        "decode_total_ms": total_ms,
        "loudness_ms": mono.loudness_ms,
    }))
}

fn thread_cpu_ms() -> f64 {
    let text = std::fs::read_to_string("/proc/thread-self/schedstat").unwrap_or_default();
    text.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 1e6
}

fn process_cpu_ms() -> f64 {
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = text.rsplit_once(')').map_or("", |(_, rest)| rest).split_whitespace().collect();
    let ticks = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) * 10.0
}

fn meter_cpu(path: &Path) -> Result<serde_json::Value> {
    let mono = decode_mono(path)?;
    let stereo: Vec<f32> = (0..mono.samples.len()).flat_map(|i| [mono.samples[i], 0.8 * mono.samples[i.saturating_sub(37)]]).collect();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = thread_cpu_ms();
        let mut meter = Meter::new(mono.rate, default_channel_weights(2));
        for chunk in stereo.chunks(2 * 1152) {
            meter.push_interleaved(chunk);
        }
        meter.finish();
        best = best.min(thread_cpu_ms() - t);
    }
    Ok(json!({ "path": path, "duration_s": mono.samples.len() as f64 / mono.rate as f64, "meter_cpu_ms": best }))
}

fn main() -> Result<()> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut full_threads: Option<usize> = None;
    let mut cpu = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--full" => full_threads = Some(args.next().context("--full THREADS")?.parse()?),
            "--cpu" => cpu = true,
            "--list" => {
                let text = std::fs::read_to_string(args.next().context("--list file")?)?;
                paths.extend(text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let engine = match full_threads {
        Some(_) => Some(BeatEngine::load(&locate_models(None).context("models")?, BEAT_MODEL, false)?),
        None => None,
    };
    let cancel = AtomicBool::new(false);
    for path in &paths {
        let doc = match (&engine, full_threads) {
            (Some(engine), Some(threads)) if cpu => {
                let mut cpu_ms = [0.0; 2];
                for (slot, loudness) in cpu_ms.iter_mut().zip([false, true]) {
                    let cfg = Config { threads, loudness, ..Config::default() };
                    let t = process_cpu_ms();
                    analyze_controlled(engine, path, &cfg, &Control::new(&cancel))?;
                    *slot = process_cpu_ms() - t;
                }
                Ok(json!({ "path": path, "pipeline_cpu_ms_off": cpu_ms[0], "pipeline_cpu_ms_on": cpu_ms[1] }))
            }
            (Some(engine), Some(threads)) => {
                let cfg = Config { threads, loudness: true, ..Config::default() };
                analyze_controlled(engine, path, &cfg, &Control::new(&cancel)).map(|a| a.to_json(path, None))
            }
            _ if cpu => meter_cpu(path),
            _ => decode_only(path),
        };
        match doc {
            Ok(doc) => println!("{doc}"),
            Err(e) => println!("{}", json!({ "path": path, "error": e.to_string() })),
        }
    }
    Ok(())
}
