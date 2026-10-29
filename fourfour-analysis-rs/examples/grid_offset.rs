use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use fourfour_analysis::analyze::{analyze, Config};
use fourfour_analysis::bt::BeatEngine;
use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, BEAT_MODEL};
use fourfour_analysis::fit::{finalize_beats, fit_track, FitParams, Window, FPS};
use fourfour_analysis::mp3::{self, DelayRule};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde_json::{json, Value};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

const WIN: usize = 512;
const HOP: usize = 32;
const SEARCH_S: f64 = 0.05;

struct Flux {
    values: Vec<f64>,
    hop_s: f64,
    centre_s: f64,
    floor: f64,
}

fn flux(samples: &[f32], rate: u32) -> Flux {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WIN);
    let window: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WIN as f32).cos()).collect();
    let bins = WIN / 2;
    let mut prev = vec![0f32; bins];
    let mut buf = vec![Complex::new(0f32, 0f32); WIN];
    let mut values = Vec::with_capacity(samples.len() / HOP + 1);
    let mut start = 0;
    while start + WIN <= samples.len() {
        for i in 0..WIN {
            buf[i] = Complex::new(samples[start + i] * window[i], 0.0);
        }
        fft.process(&mut buf);
        let mut sum = 0f64;
        for b in 1..bins {
            let m = (1.0 + 100.0 * buf[b].norm()).ln();
            let d = m - prev[b];
            if d > 0.0 {
                sum += d as f64;
            }
            prev[b] = m;
        }
        values.push(sum);
        start += HOP;
    }
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    Flux { values, hop_s: HOP as f64 / rate as f64, centre_s: (WIN as f64 / 2.0 - HOP as f64 / 2.0) / rate as f64, floor }
}

impl Flux {
    fn time(&self, i: f64) -> f64 {
        i * self.hop_s + self.centre_s
    }

    fn onset_near(&self, t: f64, min_contrast: f64) -> Option<f64> {
        let lo = (((t - SEARCH_S - self.centre_s) / self.hop_s).floor().max(1.0)) as usize;
        let hi = (((t + SEARCH_S - self.centre_s) / self.hop_s).ceil() as usize).min(self.values.len().saturating_sub(2));
        if lo >= hi {
            return None;
        }
        let i = (lo..=hi).max_by(|&a, &b| self.values[a].total_cmp(&self.values[b]))?;
        if i == lo || i == hi || self.values[i] < min_contrast * self.floor.max(1e-9) {
            return None;
        }
        let (a, b, c) = (self.values[i - 1], self.values[i], self.values[i + 1]);
        let den = a - 2.0 * b + c;
        let frac = if den.abs() > 1e-12 { 0.5 * (a - c) / den } else { 0.0 };
        Some(self.time(i as f64 + frac.clamp(-0.5, 0.5)))
    }
}

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

fn decode_trimmed(path: &Path) -> Result<Vec<f32>> {
    let mss = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())?;
    let track = format.first_track_known_codec(TrackType::Audio).ok_or_else(|| anyhow!("no audio track"))?;
    let id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| anyhow!("no params"))?;
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default())?;
    let mut out = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id != id {
            continue;
        }
        if let Ok(decoded) = decoder.decode(&packet) {
            let ch = decoded.spec().channels().count();
            decoded.copy_to_vec_interleaved(&mut scratch);
            out.extend(scratch.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
        }
    }
    Ok(out)
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) })
}

fn offsets_ms(grid_s: &[f64], onsets: &[(f64, f64)]) -> Vec<f64> {
    onsets
        .iter()
        .filter_map(|&(probe, onset)| {
            let j = grid_s.partition_point(|&g| g < probe);
            let near = [j.wrapping_sub(1), j].into_iter().filter(|&k| k < grid_s.len()).min_by(|&a, &b| (grid_s[a] - onset).abs().total_cmp(&(grid_s[b] - onset).abs()))?;
            let d = (grid_s[near] - onset) * 1000.0;
            (d.abs() <= SEARCH_S * 1000.0).then_some(d)
        })
        .collect()
}

fn track(id: u64, reference: &Value, dump: &Path, contrast: f64, rule: DelayRule, heard: &str) -> Result<Value> {
    let heard_is_untrimmed = heard == "untrimmed";
    let path = PathBuf::from(reference["path"].as_str().ok_or_else(|| anyhow!("no path"))?);
    let doc: Value = serde_json::from_str(&fs::read_to_string(dump.join(format!("{id}.json")))?)?;
    let raw = fs::read(dump.join(format!("{id}.logits")))?;
    let all: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let frames = doc["frames"].as_u64().ok_or_else(|| anyhow!("frames"))? as usize;
    let (bl, dl) = (&all[..frames], &all[frames..]);
    let peaks: Vec<f64> = doc["beats"].as_array().ok_or_else(|| anyhow!("beats"))?.iter().filter_map(|x| x.as_f64()).collect();
    let duration = doc["duration_s"].as_f64().unwrap_or(0.0);
    let w = Window { offset: 0.0, len: frames as f64 / FPS };
    let mut fit = fit_track(&peaks, bl, dl, duration, &w, &FitParams::default()).ok_or_else(|| anyhow!("no fit"))?;
    if heard == "rebased" {
        let (trim, rate) = leading_trim_samples(&path)?;
        let to_decoded = (trim + mp3::leading_silence_frames(&path)) as f64 / rate as f64;
        fit.beats_s.iter_mut().for_each(|b| *b += to_decoded);
    }
    let shift = mp3::timeline_shift_s(&path, rule);
    let app = finalize_beats(&fit.beats_s, shift);

    let mono = decode_mono(&path)?;
    let (trim, trim_rate) = leading_trim_samples(&path)?;
    let trim_s = if heard_is_untrimmed { trim as f64 / trim_rate as f64 } else { 0.0 };
    let fl = flux(&mono.samples, mono.rate);
    let onsets_g: Vec<(f64, f64)> = fit.beats_s.iter().filter_map(|&b| fl.onset_near(b, contrast).map(|o| (b, o))).collect();
    let onsets_p: Vec<(f64, f64)> = onsets_g.iter().map(|&(b, o)| (b + trim_s, o + trim_s)).collect();

    let rb: Vec<f64> = reference["beats_ms"].as_array().map(|v| v.iter().filter_map(|x| x.as_f64()).map(|x| x / 1000.0).collect()).unwrap_or_default();
    let mut model_vs_onset = offsets_ms(&fit.beats_s, &onsets_g);
    let mut app_vs_heard = offsets_ms(&app, &onsets_p);
    let mut app_vs_wave = offsets_ms(&app, &onsets_g);
    let mut rb_vs_untrimmed = offsets_ms(&rb, &onsets_p);
    let mut rb_vs_trimmed = offsets_ms(&rb, &onsets_g);
    let mut app_vs_rb: Vec<f64> = rb
        .iter()
        .filter_map(|&t| {
            let j = app.partition_point(|&g| g < t);
            [j.wrapping_sub(1), j].into_iter().filter(|&k| k < app.len()).map(|k| (app[k] - t) * 1000.0).min_by(|a, b| a.abs().total_cmp(&b.abs()))
        })
        .collect();
    let max_dev = app_vs_rb.iter().fold(0f64, |m, d| m.max(d.abs()));
    let tl = mp3::scan(&path);
    let encoder = tl.and_then(|t| t.encoder).map(|e| String::from_utf8_lossy(&e).trim_end_matches('\0').trim().to_string());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    Ok(json!({
        "id": id,
        "path": path,
        "ext": ext,
        "encoder": encoder,
        "header_counts": tl.map(|t| t.header_counts),
        "lame_delay": tl.and_then(|t| t.lame_delay),
        "trim_samples": trim,
        "lead_samples": mp3::leading_silence_frames(&path),
        "rate": mono.rate,
        "shift_ms": shift * 1000.0,
        "bpm": fit.bpm,
        "ref_bpm": reference["bpm_masterdb"].as_f64(),
        "onsets": onsets_g.len(),
        "beats": fit.beats_s.len(),
        "model_vs_onset_ms": median(&mut model_vs_onset),
        "app_vs_heard_ms": median(&mut app_vs_heard),
        "app_vs_wave_ms": median(&mut app_vs_wave),
        "rb_vs_untrimmed_ms": median(&mut rb_vs_untrimmed),
        "rb_vs_trimmed_ms": median(&mut rb_vs_trimmed),
        "app_vs_rb_ms": median(&mut app_vs_rb),
        "max_dev_ms": max_dev,
    }))
}

fn run(args: &[String]) -> Result<()> {
    let reference: Value = serde_json::from_str(&fs::read_to_string(&args[0])?)?;
    let ids: Vec<u64> = BufReader::new(File::open(&args[1])?).lines().map_while(|l| l.ok()).filter_map(|l| l.trim().parse().ok()).collect();
    let out = PathBuf::from(&args[2]);
    fs::create_dir_all(&out)?;
    let workers: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);
    let models = locate_models(None).ok_or_else(|| anyhow!("model weights not found; set FOURFOUR_MODELS_DIR"))?;
    let engine = BeatEngine::load(&models, BEAT_MODEL, false)?;
    let cfg = Config { key: false, waveform: false, keep_logits: true, ..Config::default() };
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&id) = ids.get(i) else { break };
                if out.join(format!("{id}.json")).exists() {
                    continue;
                }
                let Some(path) = reference[id.to_string()]["path"].as_str() else { continue };
                match analyze(&engine, Path::new(path), &cfg) {
                    Ok(a) => {
                        let (bl, dl) = a.logits.clone().unwrap_or_default();
                        let bytes: Vec<u8> = bl.iter().chain(dl.iter()).flat_map(|x| x.to_le_bytes()).collect();
                        fs::write(out.join(format!("{id}.logits")), bytes).ok();
                        let doc = json!({"id": id, "beats": a.raw_beats_s, "bpm": a.bpm, "duration_s": a.duration_s, "final_beats_s": a.beats_s, "frames": bl.len(), "offset_s": 0.0, "mp3_delay_ms": a.mp3_delay_s * 1000.0});
                        fs::write(out.join(format!("{id}.json")), doc.to_string()).ok();
                        eprintln!("{} / {}: {}", i + 1, ids.len(), id);
                    }
                    Err(e) => eprintln!("{} / {}: {} failed: {e}", i + 1, ids.len(), id),
                }
            });
        }
    });
    Ok(())
}

fn selftest() -> Result<()> {
    let rate = 44100u32;
    let period = 0.5;
    let first = 0.3137;
    let n = (30.0 * rate as f64) as usize;
    let mut seed = 1u32;
    let mut noise = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
    };
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / rate as f64;
            let k = ((t - first) / period).floor();
            let dt = t - (first + k * period);
            let bed = 0.01 * noise();
            if k < 0.0 {
                return bed;
            }
            let body = (2.0 * std::f64::consts::PI * (50.0 + 80.0 * (-dt / 0.03).exp()) * dt).sin() * (-dt / 0.15).exp();
            let click = if dt < 0.003 { 0.5 * noise() as f64 } else { 0.0 };
            bed + (0.8 * body + click) as f32
        })
        .collect();
    let fl = flux(&samples, rate);
    let truth: Vec<f64> = (0..59).map(|k| first + k as f64 * period).collect();
    let mut d: Vec<f64> = truth.iter().filter_map(|&t| fl.onset_near(t, 4.0).map(|o| (o - t) * 1000.0)).collect();
    println!("synthetic kicks: {} of {} found, onset minus true attack start: median {:+.2} ms", d.len(), truth.len(), median(&mut d).unwrap_or(f64::NAN));
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("selftest") {
        return selftest();
    }
    if args.get(1).map(String::as_str) == Some("durations") {
        for line in fs::read_to_string(&args[2])?.lines().filter(|l| !l.trim().is_empty()) {
            let path = Path::new(line);
            let tag = pioneer_usb_writer::scanner::read_track_metadata(path, 0)?.duration_secs;
            let mono = decode_mono(path)?;
            let decoded = mono.samples.len() as f64 / mono.rate as f64;
            let trimmed = decode_trimmed(path)?;
            let g = mono.gapless_samples();
            let diff = g.iter().zip(&trimmed).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
            println!("{:+8.1} ms  tag {tag:.3} s  decoded {decoded:.3} s  gapless slice {} vs trimmed decode {} samples, max diff {diff}  {}", (decoded - tag) * 1000.0, g.len(), trimmed.len(), path.file_name().unwrap().to_string_lossy());
        }
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("run") {
        return run(&args[2..]);
    }
    if args.len() < 5 {
        return Err(anyhow!("usage: grid_offset <ref.json> <dump dir> <ids file> <out.jsonl> [workers] [contrast] [mp3 rule] [heard: untrimmed|decoded|rebased]\n       grid_offset run <ref.json> <ids file> <out dump dir> [workers]\n       grid_offset selftest"));
    }
    let reference: Value = serde_json::from_str(&fs::read_to_string(&args[1])?)?;
    let dump = PathBuf::from(&args[2]);
    let ids: Vec<u64> = BufReader::new(File::open(&args[3])?).lines().map_while(|l| l.ok()).filter_map(|l| l.trim().parse().ok()).collect();
    let workers: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(4);
    let contrast: f64 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let rule = args.get(7).and_then(|s| DelayRule::parse(s)).unwrap_or(DelayRule::ModelLead);
    let heard = args.get(8).cloned().unwrap_or_else(|| "decoded".to_string());
    let out = Mutex::new(File::create(&args[4])?);
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&id) = ids.get(i) else { break };
                let row = match track(id, &reference[id.to_string()], &dump, contrast, rule, &heard) {
                    Ok(v) => v,
                    Err(e) => json!({"id": id, "error": e.to_string()}),
                };
                let mut f = out.lock().unwrap();
                writeln!(f, "{row}").ok();
                eprintln!("{} / {}: {}", i + 1, ids.len(), id);
            });
        }
    });
    Ok(())
}
