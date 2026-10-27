use super::tempo_tests::*;
use super::tests::{decode_to_vec, write_wav};
use super::*;

use std::path::{Path, PathBuf};

use std::fmt::Write as _;

const EXCERPT_START_SECONDS: usize = 90;

struct LabTrack {
    name: &'static str,
    path: &'static str,
}

const TRACKS: [LabTrack; 3] = [
    LabTrack {
        name: "techno-process",
        path: "/home/merlin/dev/projects/fourfour/benchmark/datasets/rekordbox-testdata-2025/#02_DJ/2025/Feb-Apr/3/Process.wav",
    },
    LabTrack {
        name: "vocal-fata-morgana",
        path: "/home/merlin/dev/projects/fourfour/benchmark/datasets/rekordbox-testdata-2025/#02_DJ/2025/Feb-Apr/3/Fata Morgana.wav",
    },
    LabTrack {
        name: "pads-ocean-of-methane",
        path: "/home/merlin/dev/projects/fourfour/benchmark/datasets/rekordbox-testdata-2025/#02_DJ/2025/Feb-Apr/3/Ocean Of Methane.wav",
    },
];

fn lab_tracks() -> Vec<(&'static str, Vec<f32>)> {
    let root = std::env::var("FOURFOUR_LAB_TRACKS");
    TRACKS
        .iter()
        .map(|t| {
            let path = match &root {
                Ok(dir) => format!("{dir}/{}", Path::new(t.path).file_name().unwrap().to_str().unwrap()),
                Err(_) => t.path.to_string(),
            };
            let all = decode_to_vec(Path::new(&path), 0, SR, 2);
            let from = EXCERPT_START_SECONDS * SR as usize * 2;
            let len = 90 * SR as usize * 2;
            (t.name, all[from..(from + len).min(all.len())].to_vec())
        })
        .collect()
}

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var("FOURFOUR_LAB_OUT")
            .unwrap_or_else(|_| "/tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/feedback/wk".to_string()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn want(section: &str) -> bool {
    match std::env::var("FOURFOUR_LAB") {
        Ok(list) => list.split(',').any(|s| s == section),
        Err(_) => true,
    }
}

fn mono_smoothed_envelope(x: &[f32], window: usize) -> Vec<f32> {
    let mut env = vec![0.0f32; x.len()];
    let mut acc = 0.0f64;
    for i in 0..x.len() {
        acc += (x[i] * x[i]) as f64;
        if i >= window {
            acc -= (x[i - window] * x[i - window]) as f64;
        }
        env[i] = (acc.max(0.0) / window as f64).sqrt() as f32;
    }
    env
}

fn onsets_of(x: &[f32], count: usize) -> Vec<usize> {
    let hop = 48;
    let env = mono_smoothed_envelope(x, 240);
    let mut flux: Vec<(f32, usize)> = (hop * 4..x.len() - hop * 4)
        .step_by(hop)
        .map(|i| ((env[i + hop * 2] - env[i]).max(0.0), i))
        .collect();
    flux.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut chosen: Vec<usize> = Vec::new();
    for (_, i) in flux {
        if i < SR as usize / 2 || i > x.len() - SR as usize / 2 {
            continue;
        }
        if chosen.iter().all(|c| c.abs_diff(i) > SR as usize / 5) {
            chosen.push(i);
            if chosen.len() == count {
                break;
            }
        }
    }
    chosen.sort();
    chosen
}

struct Attack {
    rise_ms: f64,
    pre_echo: f64,
}

fn attack_at(x: &[f32], center: usize) -> Option<Attack> {
    let radius = SR as usize / 25;
    let from = center.checked_sub(radius)?;
    let to = (center + radius).min(x.len());
    let env = mono_smoothed_envelope(&x[from..to], 48);
    let (peak_at, peak) = env
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    if peak < 1e-3 {
        return None;
    }
    let walk_back = |level: f32| {
        let mut i = peak_at;
        while i > 0 && env[i] > peak * level {
            i -= 1;
        }
        i
    };
    let t10 = walk_back(0.1);
    let t90 = walk_back(0.9);
    let pre = peak_at.saturating_sub(SR as usize * 15 / 1000);
    Some(Attack {
        rise_ms: t90.saturating_sub(t10) as f64 * 1000.0 / SR as f64,
        pre_echo: env[pre] as f64 / peak as f64,
    })
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() { f64::NAN } else { v[v.len() / 2] }
}

fn heard_to_output_frame(positions: &[f64], block: usize, start: f64, frame: f64) -> Option<usize> {
    let mut previous = start;
    for (b, &end) in positions.iter().enumerate() {
        if end >= frame {
            let within = if end > previous { (frame - previous) / (end - previous) } else { 0.0 };
            return Some((b as f64 * block as f64 + within * block as f64) as usize);
        }
        previous = end;
    }
    None
}

fn hf_spikes(x: &[f32]) -> usize {
    let win = 32;
    let mut w = vec![0.0f64; x.len() / win];
    for k in 0..w.len() {
        let slice = &x[k * win..(k + 1) * win];
        let mut e = 0.0f64;
        for i in 1..slice.len() - 1 {
            let d = (slice[i + 1] - 2.0 * slice[i] + slice[i - 1]) as f64;
            e += d * d;
        }
        w[k] = (e / win as f64).sqrt();
    }
    let mut count = 0;
    for k in 16..w.len().saturating_sub(16) {
        let mut around: Vec<f64> = (k - 16..k + 16).filter(|&j| j != k).map(|j| w[j]).collect();
        around.sort_by(|a, b| a.total_cmp(b));
        let baseline = around[around.len() / 2].max(1e-6);
        if w[k] > 12.0 * baseline && w[k] > 2e-4 {
            count += 1;
        }
    }
    count
}

fn percentile_ns(timings: &[u64], p: f64) -> u64 {
    let mut sorted = timings.to_vec();
    sorted.sort();
    sorted[((sorted.len() - 1) as f64 * p) as usize]
}

fn cpu_row(label: &str, block: usize, timings: &[u64]) -> String {
    let budget_ns = block as f64 / SR as f64 * 1e9;
    let mean = timings.iter().sum::<u64>() as f64 / timings.len() as f64;
    let p99 = percentile_ns(timings, 0.99) as f64;
    let max = *timings.iter().max().unwrap() as f64;
    format!(
        "| {label} | {block} | {:.2}% | {:.2}% | {:.2}% |",
        mean / budget_ns * 100.0,
        p99 / budget_ns * 100.0,
        max / budget_ns * 100.0
    )
}

fn write_wav_f32(path: &Path, stereo: &[f32]) {
    let frames = (stereo.len() / 2) as u32;
    write_wav(path, SR, 2, frames, |n, c| stereo[(n * 2 + c as u32) as usize].clamp(-1.0, 1.0));
}

fn to_flac(wav: &Path, flac: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(wav)
        .args(["-c:a", "flac"])
        .arg(flac)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::remove_file(wav).unwrap();
}

#[test]
#[ignore]
fn lab_report_pitch_and_tempo() {
    let mut report = String::new();
    if want("pitch") {
        let mut worst_ratio: f64 = 0.0;
        let mut worst_shift: f64 = 0.0;
        for freq in [110.0f32, 220.0, 440.0, 1000.0, 3000.0, 8000.0] {
            for ratio in [0.84, 0.92, 0.98, 1.02, 1.08, 1.16, 1.5] {
                let mut lab = Lab::new(sine(freq, 12.0, 0.5), 256, Preset::DEFAULT);
                lab.set(ratio, 0.0, true);
                let out = left(&lab.run_seconds(5.0));
                let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
                worst_ratio = worst_ratio.max(cents(f, freq as f64).abs());
            }
            for semitones in [-12.0, -7.0, -5.0, -2.0, -1.0, 1.0, 2.0, 5.0, 7.0, 12.0] {
                for ratio in [1.0, 1.08] {
                    let mut lab = Lab::new(sine(freq, 12.0, 0.5), 256, Preset::DEFAULT);
                    lab.set(ratio, semitones, true);
                    let out = left(&lab.run_seconds(5.0));
                    let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
                    let expected = freq as f64 * 2f64.powf(semitones / 12.0);
                    worst_shift = worst_shift.max(cents(f, expected).abs());
                }
            }
        }
        writeln!(report, "PITCH key lock, 7 ratios x 6 freqs: worst {worst_ratio:.3} cents").unwrap();
        writeln!(report, "PITCH key shift -12..+12 st (ratio 1.0 and 1.08) x 6 freqs: worst {worst_shift:.3} cents").unwrap();

        let chirp_seconds = 20.0f64;
        let frames = (chirp_seconds * SR as f64) as usize;
        let (f0, f1) = (100.0f64, 6000.0f64);
        let k = (f1 / f0).ln() / chirp_seconds;
        let phase = |t: f64| 2.0 * std::f64::consts::PI * f0 * ((k * t).exp() - 1.0) / k;
        let mut source = Vec::with_capacity(frames * 2);
        for n in 0..frames {
            let v = (0.5 * phase(n as f64 / SR as f64).sin()) as f32;
            source.push(v);
            source.push(v);
        }
        for ratio in [0.92, 1.08, 1.16] {
            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.set(ratio, 0.0, true);
            lab.settle();
            lab.locate(0);
            lab.positions.clear();
            let out = left(&lab.run_seconds(14.0));
            let mut worst: f64 = 0.0;
            let window = SR as usize / 5;
            let mut at = SR as usize;
            while at + window < out.len() {
                let center = at + window / 2;
                let b = center / 256;
                let before = if b == 0 { 0.0 } else { lab.positions[b - 1] };
                let heard_frame = before + (lab.positions[b] - before) * (center % 256) as f64 / 256.0;
                let t_src = heard_frame / SR as f64;
                if t_src < chirp_seconds - 1.0 {
                    let expected = f0 * (k * t_src).exp();
                    let measured = zero_cross_freq(&out, at, at + window);
                    worst = worst.max(cents(measured, expected).abs());
                }
                at += window;
            }
            writeln!(report, "PITCH sweep 100 Hz..6 kHz at ratio {ratio}, key lock on: worst {worst:.2} cents (200 ms windows)").unwrap();
        }
    }

    if want("tempo") {
        let seconds = 300.0f32;
        let click_period = 0.5;
        for ratio in [0.92, 1.08, 1.16] {
            let mut lab = Lab::new(click_train(click_period, seconds), 256, Preset::DEFAULT);
            lab.set(ratio, 0.0, true);
            lab.settle();
            lab.locate(0);
            lab.positions.clear();
            let out = left(&lab.run_until_drained(seconds / ratio as f32 + 5.0));
            let clicks = click_centers(click_period, seconds);
            let peaks = peak_positions(&out, 0.03, SR as usize / 4);
            let n = peaks.len().min(clicks.len());
            let xs: Vec<f64> = clicks[..n].iter().map(|c| c / ratio).collect();
            let ys: Vec<f64> = peaks[..n].to_vec();
            let mx = xs.iter().sum::<f64>() / n as f64;
            let my = ys.iter().sum::<f64>() / n as f64;
            let slope = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum::<f64>()
                / xs.iter().map(|x| (x - mx) * (x - mx)).sum::<f64>();
            let worst_residual = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| (y - my - slope * (x - mx)).abs())
                .fold(0.0, f64::max);
            let drift_ms = (slope - 1.0).abs() * seconds as f64 / ratio * 1000.0;
            let offset_spread = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| y - x)
                .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)));
            writeln!(
                report,
                "TEMPO 5 min click grid at ratio {ratio}: {n} beats, drift over 5 min {drift_ms:.3} ms, worst beat residual {:.3} ms, beat offset spread {:.3} ms",
                worst_residual * 1000.0 / SR as f64,
                (offset_spread.1 - offset_spread.0) * 1000.0 / SR as f64
            )
            .unwrap();
        }
    }

    if want("latency") {
        let mut worst: f64 = 0.0;
        for (ratio, semitones, lock) in [
            (1.0, 0.0, true),
            (0.84, 0.0, true),
            (0.92, 0.0, true),
            (1.08, 0.0, true),
            (1.16, 0.0, true),
            (1.5, 0.0, true),
            (1.08, 3.0, true),
            (0.92, -5.0, true),
            (1.08, 0.0, false),
            (0.84, 0.0, false),
            (1.5, 0.0, false),
        ] {
            let mut err: f64 = 0.0;
            for start in [48_000 * 2 + 777, 48_000 * 5 + 13, 48_000 * 3 + 24_001] {
                err = err.max(heard_error_ms(0.5, ratio, semitones, lock, start));
            }
            writeln!(report, "LATENCY ratio {ratio} st {semitones} lock {lock}: worst playhead error after seek {err:.2} ms").unwrap();
            worst = worst.max(err);
        }
        writeln!(report, "LATENCY worst overall {worst:.2} ms").unwrap();
    }
    println!("{report}");
    std::fs::write(out_dir().join("report-pitch-tempo-latency.txt"), &report).unwrap();
}

#[test]
#[ignore]
fn lab_report_tracks() {
    let tracks = lab_tracks();
    let mut report = String::new();

    if want("transparency") {
        for (name, source) in &tracks {
            for lock in [true, false] {
                let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
                lab.set(1.0, 0.0, lock);
                let out = lab.run_seconds(60.0);
                let mismatches = (256 * 2..out.len()).filter(|&i| out[i].to_bits() != source[i].to_bits()).count();
                writeln!(report, "TRANSPARENCY {name} lock {lock}: {mismatches} differing samples of {}", out.len() - 512).unwrap();
            }
        }
    }

    if want("transients") {
        let presets = [
            ("default 0.12/0.03", Preset::DEFAULT),
            ("default split", Preset { split_computation: true, ..Preset::DEFAULT }),
            ("cheaper 0.10/0.04 split", Preset::CHEAPER),
        ];
        for (name, source) in tracks.iter().take(1) {
            let mono = left(source);
            let onsets = onsets_of(&mono, 40);
            let reference: Vec<Attack> = onsets.iter().filter_map(|&o| attack_at(&mono, o)).collect();
            let ref_rise = median(reference.iter().map(|a| a.rise_ms).collect());
            let ref_pre = median(reference.iter().map(|a| a.pre_echo).collect());
            writeln!(report, "TRANSIENTS {name}: source rise {ref_rise:.2} ms, pre-echo {ref_pre:.3}, {} onsets", reference.len()).unwrap();
            for (label, preset) in presets {
                for ratio in [0.92, 1.08] {
                    let mut lab = Lab::new(source.clone(), 256, preset);
                    lab.set(ratio, 0.0, true);
                    lab.settle();
                    lab.locate(0);
                    lab.positions.clear();
                    let out = left(&lab.run_seconds(80.0 / ratio as f32));
                    let mut rises = Vec::new();
                    let mut pres = Vec::new();
                    let mut ref_rises = Vec::new();
                    for &onset in &onsets {
                        if let Some(o) = heard_to_output_frame(&lab.positions, 256, 0.0, onset as f64) {
                            if let (Some(a), Some(r)) = (attack_at(&out, o), attack_at(&mono, onset)) {
                                rises.push(a.rise_ms * ratio);
                                pres.push(a.pre_echo);
                                ref_rises.push(r.rise_ms);
                            }
                        }
                    }
                    writeln!(
                        report,
                        "TRANSIENTS {name} {label} ratio {ratio}: rise x{:.2} of source (time-scale corrected), pre-echo {:.3} (source {ref_pre:.3}), {} onsets matched",
                        median(rises.clone()) / median(ref_rises.clone()),
                        median(pres),
                        rises.len()
                    )
                    .unwrap();
                }
            }
        }
    }

    if want("clicks") {
        for (name, source) in &tracks {
            let baseline = {
                let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
                lab.set(1.0, 0.0, true);
                hf_spikes(&left(&lab.run_seconds(40.0)))
            };
            let mut scenarios: Vec<(String, usize)> = Vec::new();

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.settle();
            let mut out = Vec::new();
            for b in 0..(10 * SR as usize / 256) {
                let t = b as f64 * 256.0 / SR as f64;
                lab.set(0.92 + 0.016 * t, 0.0, true);
                lab.step(&mut out);
            }
            scenarios.push(("tempo sweep -8..+8 % over 10 s, key lock".into(), hf_spikes(&left(&out))));

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.settle();
            let mut out = Vec::new();
            for b in 0..(10 * SR as usize / 256) {
                let t = b as f64 * 256.0 / SR as f64;
                lab.set(0.92 + 0.016 * t, 0.0, false);
                lab.step(&mut out);
            }
            scenarios.push(("tempo sweep -8..+8 % over 10 s, varispeed".into(), hf_spikes(&left(&out))));

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.settle();
            let mut out = Vec::new();
            for b in 0..(12 * SR as usize / 256) {
                let t = b as f64 * 256.0 / SR as f64;
                lab.set(1.0, [0.0, 2.0, -3.0, 5.0, 0.0, -1.0][(t / 2.0) as usize % 6], true);
                lab.step(&mut out);
            }
            scenarios.push(("key changes every 2 s (0,+2,-3,+5,0,-1)".into(), hf_spikes(&left(&out))));

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.set(1.08, 0.0, true);
            lab.settle();
            let mut out = Vec::new();
            for i in 0..8 {
                lab.locate(SR as usize * (3 + i * 7));
                lab.step(&mut out);
                out.extend(lab.run_seconds(2.0));
            }
            scenarios.push(("8 seeks/cue jumps at +8 %".into(), hf_spikes(&left(&out))));

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.set(1.0, 0.0, true);
            lab.settle();
            let mut out = Vec::new();
            for _ in 0..8 {
                lab.locate(SR as usize * 20);
                out.extend(lab.run_seconds(1.9));
            }
            scenarios.push(("8 loop jumps at ratio 1.0".into(), hf_spikes(&left(&out))));

            let mut out = Vec::new();
            for i in 0..4 {
                let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
                lab.set(1.0 + 0.04 * i as f64, 0.0, true);
                lab.locate(SR as usize * 5);
                out.extend(lab.run_seconds(2.0));
            }
            scenarios.push(("4 track loads (fresh engine) at +0..12 %".into(), hf_spikes(&left(&out))));

            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            lab.set(1.08, 0.0, true);
            lab.settle();
            let mut out = Vec::new();
            for _ in 0..6 {
                out.extend(lab.run_seconds(1.5));
                let position = lab.position_frames() as usize;
                lab.locate(position);
                out.extend(lab.run_seconds(0.5));
            }
            scenarios.push(("6 pause/resume cycles at +8 %".into(), hf_spikes(&left(&out))));

            writeln!(report, "CLICKS {name}: identity baseline {baseline} spikes in 40 s").unwrap();
            for (label, count) in scenarios {
                writeln!(report, "CLICKS {name} | {label}: {count} spikes").unwrap();
            }
        }
    }

    if want("cpu") {
        writeln!(report, "\n| scenario | block | mean | p99 | max |\n|---|---|---|---|---|").unwrap();
        let (_, source) = &tracks[0];
        for block in [128usize, 256, 512] {
            let scenarios: [(&str, f64, f64, bool); 8] = [
                ("identity, key lock on (shadow stretcher)", 1.0, 0.0, true),
                ("identity, key lock off", 1.0, 0.0, false),
                ("+8 %, key lock", 1.08, 0.0, true),
                ("-16 %, key lock", 0.84, 0.0, true),
                ("+50 %, key lock", 1.5, 0.0, true),
                ("+8 %, +3 st", 1.08, 3.0, true),
                ("-12 st", 1.0, -12.0, true),
                ("+8 %, varispeed", 1.08, 0.0, false),
            ];
            for (label, ratio, semitones, lock) in scenarios {
                let mut lab = Lab::new(source.clone(), block, Preset::DEFAULT);
                lab.set(ratio, semitones, lock);
                lab.settle();
                lab.timings_ns.clear();
                lab.run_seconds(40.0);
                writeln!(report, "{}", cpu_row(label, block, &lab.timings_ns)).unwrap();
            }
            let mut lab = Lab::new(source.clone(), block, Preset::DEFAULT);
            lab.set(1.08, 0.0, true);
            lab.settle();
            lab.timings_ns.clear();
            for i in 0..40 {
                lab.locate(SR as usize * (2 + i));
                lab.run_seconds(1.0);
            }
            writeln!(report, "{}", cpu_row("40 seeks at +8 % (priming bursts included)", block, &lab.timings_ns)).unwrap();
        }
    }
    println!("{report}");
    std::fs::write(out_dir().join("report-tracks.txt"), &report).unwrap();
}

#[test]
#[ignore]
fn lab_render_listening_set() {
    let tracks = lab_tracks();
    let dir = out_dir().join("renders");
    std::fs::create_dir_all(&dir).unwrap();
    let mut index = String::from("# Listening renders (fourfour tempo/key engine, Signalsmith Stretch + in-tree varispeed)\n\nEach track: 30 s excerpt starting 1:30 into the track (the engine renders offline through the exact real-time callback path, 256-frame blocks at 48 kHz).\n\n");
    let cases: [(&str, &str, f64, f64, bool); 8] = [
        ("original", "untouched (identity, bit-exact)", 1.0, 0.0, true),
        ("plus8_lock", "+8 % tempo, key lock on (pitch stays)", 1.08, 0.0, true),
        ("minus8_lock", "-8 % tempo, key lock on (pitch stays)", 0.92, 0.0, true),
        ("plus16_lock", "+16 % tempo, key lock on", 1.16, 0.0, true),
        ("plus2st", "+2 semitones, tempo unchanged", 1.0, 2.0, true),
        ("minus2st", "-2 semitones, tempo unchanged", 1.0, -2.0, true),
        ("plus8_varispeed", "+8 % tempo, key lock off (pitch rises like a turntable)", 1.08, 0.0, false),
        ("sweep", "tempo sweep -8 % to +8 % over 10 s, then holds, key lock on", 1.0, 0.0, true),
    ];
    for (name, source) in &tracks {
        writeln!(index, "## {name}\n").unwrap();
        for (tag, description, ratio, semitones, lock) in cases {
            let mut lab = Lab::new(source.clone(), 256, Preset::DEFAULT);
            let mut out = Vec::new();
            if tag == "sweep" {
                lab.set(0.92, 0.0, true);
                lab.settle();
                lab.locate(0);
                lab.positions.clear();
                for b in 0..(30 * SR as usize / 256) {
                    let t = b as f64 * 256.0 / SR as f64;
                    lab.set((0.92 + 0.016 * t).min(1.08), 0.0, true);
                    lab.step(&mut out);
                }
            } else {
                lab.set(ratio, semitones, lock);
                lab.settle();
                lab.locate(0);
                out = lab.run_seconds(30.0);
            }
            out.truncate(30 * SR as usize * 2);
            let wav = dir.join(format!("{name}__{tag}.wav"));
            write_wav_f32(&wav, &out);
            to_flac(&wav, &dir.join(format!("{name}__{tag}.flac")));
            writeln!(index, "- `{name}__{tag}.flac`: {description}").unwrap();
        }
        let reference = dir.join(format!("{name}__ref-rubberband"));
        std::fs::create_dir_all(&reference).unwrap();
        let src_wav = dir.join(format!("{name}__src.wav"));
        write_wav_f32(&src_wav, &source[..30 * SR as usize * 2]);
        for (tag, tempo, pitch) in [("plus8_lock", 1.08, 1.0), ("minus8_lock", 0.92, 1.0), ("plus16_lock", 1.16, 1.0), ("plus2st", 1.0, 2f64.powf(2.0 / 12.0)), ("minus2st", 1.0, 2f64.powf(-2.0 / 12.0))] {
            let target = dir.join(format!("{name}__{tag}__REF-rubberband.flac"));
            let filter = format!("rubberband=tempo={tempo}:pitch={pitch}:pitchq=quality:transients=crisp:window=standard");
            let status = std::process::Command::new("ffmpeg")
                .args(["-y", "-v", "error", "-i"])
                .arg(&src_wav)
                .args(["-af", &filter, "-c:a", "flac"])
                .arg(&target)
                .status()
                .unwrap();
            assert!(status.success());
            writeln!(index, "- `{name}__{tag}__REF-rubberband.flac`: reference only, Rubber Band (R2, via ffmpeg) at the same setting, never linked into the app").unwrap();
        }
        std::fs::remove_dir_all(&reference).unwrap();
        std::fs::remove_file(&src_wav).unwrap();
        writeln!(index).unwrap();
    }
    std::fs::write(dir.join("index.md"), index).unwrap();
}
