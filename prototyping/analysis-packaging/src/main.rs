use std::path::{Path, PathBuf};
use std::time::Instant;

use beat_this::{calculate_bpm, load_audio, BeatThis, RtenRuntime};
use rustfft::{num_complex::Complex, FftPlanner};
use serde_json::json;

const FFT_SIZE: usize = 2048;
const DETAIL_PER_SECOND: f64 = 150.0;
const OVERVIEW_ENTRIES: usize = 1200;
const PWAV_ENTRIES: usize = 400;

struct Args {
    beat_model: PathBuf,
    mel_model: PathBuf,
    skip_beats: bool,
    files: Vec<PathBuf>,
}

fn parse_args() -> Args {
    let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("models");
    let mut beat_model = models.join("beat_this.onnx");
    let mut skip_beats = false;
    let mut files = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--small" => beat_model = models.join("beat_this_small.onnx"),
            "--no-beats" => skip_beats = true,
            "--beat-model" => beat_model = PathBuf::from(it.next().expect("--beat-model needs a path")),
            _ => files.push(PathBuf::from(a)),
        }
    }
    Args { beat_model, mel_model: models.join("mel_spectrogram.onnx"), skip_beats, files }
}

fn camelot(key: &stratum_dsp::analysis::result::Key) -> String {
    use stratum_dsp::analysis::result::Key;
    const MAJOR: [&str; 12] = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"];
    const MINOR: [&str; 12] = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"];
    match key {
        Key::Major(i) => MAJOR[*i as usize % 12].to_string(),
        Key::Minor(i) => MINOR[*i as usize % 12].to_string(),
    }
}

fn band_entries(mono: &[f32], sr: u32, entries: usize) -> Vec<[u8; 3]> {
    let hop = (mono.len() / entries).max(1);
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
    let bin_hz = sr as f32 / FFT_SIZE as f32;
    let bin_of = |hz: f32| (hz / bin_hz).ceil() as usize;
    let (low_lo, low_hi, high_lo) = (bin_of(20.0), bin_of(250.0), bin_of(4000.0));
    let mid_lo = low_hi;
    let mid_hi = high_lo;
    let nbins = FFT_SIZE / 2 + 1;

    let mut raw = Vec::with_capacity(entries);
    let (mut max_l, mut max_m, mut max_h) = (0f32, 0f32, 0f32);
    let mut buf = vec![Complex { re: 0f32, im: 0f32 }; FFT_SIZE];
    for i in 0..entries.min(mono.len() / hop) {
        let chunk = &mono[i * hop..(i + 1) * hop];
        for (j, c) in buf.iter_mut().enumerate() {
            *c = Complex { re: chunk.get(j).copied().unwrap_or(0.0), im: 0.0 };
        }
        fft.process(&mut buf);
        let mean = |lo: usize, hi: usize| {
            let hi = hi.min(nbins);
            if hi <= lo { 0.0 } else { buf[lo..hi].iter().map(|c| c.norm()).sum::<f32>() / (hi - lo) as f32 }
        };
        let (l, m, h) = (mean(low_lo, low_hi), mean(mid_lo, mid_hi), mean(high_lo, nbins));
        max_l = max_l.max(l);
        max_m = max_m.max(m);
        max_h = max_h.max(h);
        raw.push((l, m, h));
    }
    let scale = |v: f32, max: f32, top: f32| if max > 0.0 { ((v / max) * top) as u8 } else { 0 };
    let mut out: Vec<[u8; 3]> = raw
        .into_iter()
        .map(|(l, m, h)| [scale(l, max_l, 96.0), scale(m, max_m, 64.0), scale(h, max_h, 48.0)])
        .collect();
    out.resize(entries, [0, 0, 0]);
    out
}

fn pwav_preview(mono: &[f32]) -> [u8; PWAV_ENTRIES] {
    let chunk = (mono.len() / PWAV_ENTRIES).max(1);
    let mut rms = [0f32; PWAV_ENTRIES];
    for (i, r) in rms.iter_mut().enumerate() {
        let start = i * chunk;
        if start >= mono.len() {
            break;
        }
        let end = (start + chunk).min(mono.len());
        let c = &mono[start..end];
        *r = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
    }
    let max = rms.iter().cloned().fold(0f32, f32::max);
    let mut out = [0u8; PWAV_ENTRIES];
    if max > 0.0 {
        for (o, r) in out.iter_mut().zip(rms) {
            let n = r / max;
            *o = (((n * 7.0) as u8) << 5) | ((n * 31.0) as u8 & 0x1F);
        }
    }
    out
}

fn main() {
    let args = parse_args();
    let mut beat_tracker = if args.skip_beats {
        None
    } else {
        let t = Instant::now();
        let bt = BeatThis::new(&RtenRuntime, &args.mel_model, &args.beat_model).expect("load beat models");
        eprintln!("beat model load: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        Some(bt)
    };

    for path in &args.files {
        let total = Instant::now();

        let t = Instant::now();
        let audio = load_audio(path, 44100).expect("decode");
        let decode_ms = t.elapsed().as_secs_f64() * 1000.0;
        let duration_s = audio.samples.len() as f64 / 44100.0;

        let t = Instant::now();
        let stratum = stratum_dsp::analyze_audio(&audio.samples, 44100, stratum_dsp::AnalysisConfig::default())
            .expect("stratum analysis");
        let stratum_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let pwav = pwav_preview(&audio.samples);
        let detail = band_entries(&audio.samples, 44100, (duration_s * DETAIL_PER_SECOND) as usize);
        let overview = band_entries(&audio.samples, 44100, OVERVIEW_ENTRIES);
        let waveform_ms = t.elapsed().as_secs_f64() * 1000.0;

        let beats = beat_tracker.as_mut().map(|bt| {
            let t = Instant::now();
            let a = bt.analyze_audio(&audio.samples, 44100).expect("beat analysis");
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let bpm = calculate_bpm(&a);
            json!({
                "ms": ms,
                "bpm": bpm,
                "beats": a.beats.len(),
                "downbeats": a.downbeats.len(),
                "first_beat_s": a.beats.first(),
                "first_downbeat_s": a.downbeats.first(),
            })
        });

        let first_beats: Vec<f32> = stratum.beat_grid.beats.iter().take(4).copied().collect();
        println!(
            "{}",
            json!({
                "path": path,
                "duration_s": duration_s,
                "ms": {
                    "decode": decode_ms,
                    "stratum": stratum_ms,
                    "waveform": waveform_ms,
                    "total": total.elapsed().as_secs_f64() * 1000.0,
                },
                "stratum": {
                    "bpm": stratum.bpm,
                    "bpm_confidence": stratum.bpm_confidence,
                    "key": stratum.key.name(),
                    "camelot": camelot(&stratum.key),
                    "key_confidence": stratum.key_confidence,
                    "beats": stratum.beat_grid.beats.len(),
                    "first_beats_s": first_beats,
                    "grid_stability": stratum.grid_stability,
                },
                "beat_this": beats,
                "waveform": {
                    "pwav_len": pwav.len(),
                    "pwav_head": &pwav[..8],
                    "detail_entries": detail.len(),
                    "overview_entries": overview.len(),
                    "detail_head": &detail[..detail.len().min(3)],
                },
            })
        );
    }
}
