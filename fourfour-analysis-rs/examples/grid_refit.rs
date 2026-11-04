use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use fourfour_analysis::fit::{finalize_beats, fit_track, FitParams, TempoSearch, Window, FPS};
use fourfour_analysis::mp3::{self, DelayRule};
use serde_json::{json, Value};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

fn leading_trim_samples(path: &Path) -> Result<(u32, u32)> {
    let mss = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())?;
    let track = format.first_track_known_codec(TrackType::Audio).ok_or_else(|| anyhow!("no audio track"))?;
    let rate = track.codec_params.as_ref().and_then(|p| p.audio()).and_then(|a| a.sample_rate).unwrap_or(44100);
    Ok((track.delay.unwrap_or(0), rate))
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn track(id: u64, reference: &Value, dump: &Path, params: &FitParams) -> Result<Value> {
    let path = PathBuf::from(reference["path"].as_str().ok_or_else(|| anyhow!("no path"))?);
    let doc: Value = serde_json::from_str(&fs::read_to_string(dump.join(format!("{id}.json")))?)?;
    let raw = fs::read(dump.join(format!("{id}.logits")))?;
    let all: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let frames = doc["frames"].as_u64().ok_or_else(|| anyhow!("frames"))? as usize;
    let (bl, dl) = (&all[..frames], &all[frames..]);
    let peaks: Vec<f64> = doc["beats"].as_array().ok_or_else(|| anyhow!("beats"))?.iter().filter_map(|x| x.as_f64()).collect();
    let duration = doc["duration_s"].as_f64().unwrap_or(0.0);
    let w = Window { offset: 0.0, len: frames as f64 / FPS };
    let fit = fit_track(&peaks, bl, dl, duration, &w, params).ok_or_else(|| anyhow!("no fit"))?;
    let (trim, rate) = leading_trim_samples(&path)?;
    let to_decoded = (trim + mp3::leading_silence_frames(&path)) as f64 / rate as f64;
    let rebased: Vec<f64> = fit.beats_s.iter().map(|b| b + to_decoded).collect();
    let app = finalize_beats(&rebased, mp3::timeline_shift_s(&path, DelayRule::ModelLead));

    let rb: Vec<f64> = reference["beats_ms"].as_array().map(|v| v.iter().filter_map(|x| x.as_f64()).map(|x| x / 1000.0).collect()).unwrap_or_default();
    let offs: Vec<(f64, f64)> = rb
        .iter()
        .filter_map(|&t| {
            let j = app.partition_point(|&g| g < t);
            [j.wrapping_sub(1), j].into_iter().filter(|&k| k < app.len()).map(|k| (app[k] - t) * 1000.0).min_by(|a, b| a.abs().total_cmp(&b.abs())).map(|d| (t, d))
        })
        .collect();
    let max_dev = offs.iter().fold(0f64, |m, d| m.max(d.1.abs()));
    let mut signed: Vec<f64> = offs.iter().map(|d| d.1).collect();
    let med = median(&mut signed);
    let n = offs.len() as f64;
    let (mx, my) = (offs.iter().map(|d| d.0).sum::<f64>() / n, offs.iter().map(|d| d.1).sum::<f64>() / n);
    let sxx: f64 = offs.iter().map(|d| (d.0 - mx).powi(2)).sum();
    let sxy: f64 = offs.iter().map(|d| (d.0 - mx) * (d.1 - my)).sum();
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let span = offs.last().map_or(0.0, |d| d.0) - offs.first().map_or(0.0, |d| d.0);
    let detrended = offs.iter().map(|d| (d.1 - (my + slope * (d.0 - mx))).abs()).fold(0f64, f64::max);
    let constant = rb.len() > 2 && {
        let k = rb.len() - 1;
        let p = (rb[k] - rb[0]) / k as f64;
        rb.iter().enumerate().all(|(i, t)| (t - (rb[0] + p * i as f64)).abs() <= 0.005)
    };
    Ok(json!({
        "id": id,
        "ext": path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
        "bpm": fit.bpm,
        "ref_bpm": reference["bpm_masterdb"].as_f64(),
        "ref_constant": constant,
        "beats": app.len(),
        "med_offset_ms": med,
        "max_dev_ms": max_dev,
        "drift_ms": slope * span,
        "detrended_max_ms": detrended,
        "fit_us": fit.fit_us,
    }))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err(anyhow!("usage: grid_refit <ref.json> <dump dir> <ids file> <out.jsonl> [workers] [nosnap]; env R79_SCAN_PCT, R79_TEMPO_SEARCH (line|global), R79_GLOBAL_PCT, R79_SNAP_PHASE_MS, R79_SNAP_WIDE override FitParams"));
    }
    let reference: Value = serde_json::from_str(&fs::read_to_string(&args[0])?)?;
    let dump = PathBuf::from(&args[1]);
    let ids: Vec<u64> = BufReader::new(File::open(&args[2])?).lines().map_while(|l| l.ok()).filter_map(|l| l.trim().parse().ok()).collect();
    let out = Mutex::new(File::create(&args[3])?);
    let workers: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut params = FitParams::default();
    if let Some(v) = std::env::var("R79_SCAN_PCT").ok().and_then(|s| s.parse().ok()) {
        params.scan_pct = v;
    }
    if let Some(v) = std::env::var("R79_SNAP_PHASE_MS").ok().and_then(|s| s.parse().ok()) {
        params.snap_phase_ms = v;
    }
    match std::env::var("R79_TEMPO_SEARCH").as_deref() {
        Ok("line") => params.tempo_search = TempoSearch::NearLine,
        Ok("global") => params.tempo_search = TempoSearch::LineAndGlobalPeak,
        _ => {}
    }
    if let Some(v) = std::env::var("R79_GLOBAL_PCT").ok().and_then(|s| s.parse().ok()) {
        params.global_pct = v;
    }
    if let Some(v) = std::env::var("R79_SNAP_WIDE").ok().and_then(|s| s.parse().ok()) {
        params.snap_wide = v;
    }
    if std::env::var("R101_HALF_TIME").as_deref() == Ok("off") {
        params.half_time_band = (0.0, 0.0);
    }
    if args.get(5).is_some_and(|s| s == "nosnap") {
        params.snap_tol = 0.0;
    }
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&id) = ids.get(i) else { break };
                if !dump.join(format!("{id}.json")).exists() {
                    continue;
                }
                match track(id, &reference[id.to_string()], &dump, &params) {
                    Ok(row) => {
                        writeln!(out.lock().unwrap(), "{row}").ok();
                    }
                    Err(e) => eprintln!("{id}: {e}"),
                }
            });
        }
    });
    Ok(())
}
