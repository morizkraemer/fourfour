/// pioneer-cli — quick-test CLI for analyzing a single track and writing to USB.
///
/// Usage:
///   cargo run --bin pioneer-cli -- --audio <file> --usb <path> [--wipe] [--2ex]
///
/// Example:
///   cargo run --bin pioneer-cli -- \
///     --audio ~/Music/track.flac \
///     --usb /Volumes/ORANGE \
///     --wipe
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

fn resolve_python() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let venv_python = Path::new(manifest_dir)
        .parent()
        .unwrap_or(Path::new("."))
        .join("analysis/.venv/bin/python");
    if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        "python3".to_string()
    }
}

fn main() {
    let matches = clap::Command::new("pioneer-cli")
        .version(pioneer_usb_writer::VERSION)
        .about("Analyze a single track and write to USB")
        .arg(
            clap::Arg::new("audio")
                .long("audio")
                .short('a')
                .value_name("FILE")
                .required(true)
                .help("Path to the audio file to analyze"),
        )
        .arg(
            clap::Arg::new("usb")
                .long("usb")
                .short('u')
                .value_name("PATH")
                .required(true)
                .help("Path to the USB drive (e.g. /Volumes/ORANGE)"),
        )
        .arg(
            clap::Arg::new("wipe")
                .long("wipe")
                .short('w')
                .action(clap::ArgAction::SetTrue)
                .help("Wipe all Pioneer data from the USB before writing"),
        )
        .get_matches();

    let audio_path = matches.get_one::<String>("audio").unwrap();
    let usb_path = matches.get_one::<String>("usb").unwrap();
    let wipe = matches.get_flag("wipe");

    if let Err(e) = run(audio_path, usb_path, wipe) {
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}

fn run(audio_path: &str, usb_path: &str, wipe: bool) -> Result<()> {
    let audio = PathBuf::from(audio_path).canonicalize()
        .with_context(|| format!("Cannot resolve audio path: {audio_path}"))?;
    let usb = PathBuf::from(usb_path);

    if !audio.exists() {
        anyhow::bail!("Audio file not found: {}", audio.display());
    }
    if !usb.exists() {
        anyhow::bail!("USB path not found: {}", usb.display());
    }

    // 1. Optional wipe
    if wipe {
        println!("Wiping Pioneer data from {}…", usb.display());
        wipe_usb(&usb)?;
    }

    // 2. Scan the audio file
    println!("Scanning {}…", audio.display());
    let mut tracks = pioneer_usb_writer::scanner::scan_files(&[audio.clone()])
        .with_context(|| format!("Failed to scan {}", audio.display()))?;
    if tracks.is_empty() {
        anyhow::bail!("No tracks found in {}", audio.display());
    }
    let track = &mut tracks[0];
    track.id = 1; // single-track export
    println!("  Found: {} - {} ({})", track.artist, track.title, track.usb_path);

    // 3. Analyze with Python backend
    println!("Analyzing…");
    let python = resolve_python();
    let output = std::process::Command::new(&python)
        .args([
            "-m",
            "fourfour_analysis",
            "analyze",
            audio.to_str().unwrap(),
            "--json",
            "--backend",
            "deeprhythm_essentia",
        ])
        .current_dir(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(Path::new("."))
                .join("analysis/src"),
        )
        .output()
        .with_context(|| format!("Failed to run Python analyzer: {python}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Python analysis failed:\n{stderr}");
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse Python JSON output")?;

    let analysis = python_result_to_analysis(&json);
    println!(
        "  BPM: {:.1}, Key: {}, Beats: {}, Cues: {}",
        analysis.bpm,
        analysis.key,
        analysis.beat_grid.beats.len(),
        analysis.cue_points.len()
    );

    // 4. Write to USB
    println!("Writing to {}…", usb.display());
    pioneer_usb_writer::writer::filesystem::write_usb(
        &usb,
        &tracks,
        &[analysis],
        &[],
    )
    .context("Failed to write USB")?;

    println!("Done. Eject and test on CDJ.");
    Ok(())
}

fn wipe_usb(path: &Path) -> Result<()> {
    let pioneer = path.join("PIONEER");
    let contents = path.join("Contents");
    if pioneer.exists() {
        std::fs::remove_dir_all(&pioneer)
            .with_context(|| format!("Failed to remove {}", pioneer.display()))?;
    }
    if contents.exists() {
        std::fs::remove_dir_all(&contents)
            .with_context(|| format!("Failed to remove {}", contents.display()))?;
    }
    Ok(())
}

fn python_result_to_analysis(json: &serde_json::Value) -> pioneer_usb_writer::models::AnalysisResult {
    let result = json
        .as_object()
        .and_then(|obj| obj.values().next())
        .filter(|v| v.is_object())
        .unwrap_or(json);

    let bpm = result.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let key = result.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let beats: Vec<pioneer_usb_writer::models::Beat> = result
        .get("beats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let mut out = Vec::new();
            for b in arr {
                let bar_pos = b.get("bar_position").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                let ts = b.get("time_seconds");
                if let Some(time_secs) = ts.and_then(|v| v.as_f64()) {
                    out.push(pioneer_usb_writer::models::Beat {
                        bar_position: bar_pos,
                        time_ms: (time_secs * 1000.0) as u32,
                        tempo: (bpm * 100.0) as u32,
                    });
                } else if let Some(times) = ts.and_then(|v| v.as_array()) {
                    for t in times {
                        if let Some(time_secs) = t.as_f64() {
                            out.push(pioneer_usb_writer::models::Beat {
                                bar_position: bar_pos,
                                time_ms: (time_secs * 1000.0) as u32,
                                tempo: (bpm * 100.0) as u32,
                            });
                        }
                    }
                }
            }
            out.sort_by_key(|b| b.time_ms);
            out
        })
        .unwrap_or_default();

    let mut waveform_data = [0u8; 400];
    if let Some(arr) = result.get("waveform_peaks").and_then(|v| v.as_array()) {
        let n = arr.len();
        if n > 0 {
            for i in 0..400usize {
                let src = &arr[i * n / 400];
                let max_val = src.get("max_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let height = (max_val.abs().min(1.0) * 31.0) as u8;
                waveform_data[i] = height << 3;
            }
        }
    }

    let peaks = result.get("waveform_peaks").and_then(|v| v.as_array());
    let color_waveform = result
        .get("waveform_colors")
        .and_then(|v| v.as_array())
        .filter(|arr| !arr.is_empty())
        .map(|arr| {
            let detail: Vec<[u8; 3]> = arr
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let r = c.get("r").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let g = c.get("g").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b = c.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let amp = peaks
                        .and_then(|p| p.get(i))
                        .map(|p| {
                            let max_v = p.get("max_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let min_v = p.get("min_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            ((max_v - min_v) / 2.0).clamp(0.0, 1.0)
                        })
                        .unwrap_or(1.0);
                    [
                        (r * amp).round().min(255.0) as u8,
                        (g * amp).round().min(255.0) as u8,
                        (b * amp).round().min(255.0) as u8,
                    ]
                })
                .collect();
            let n = detail.len();
            let overview: Vec<[u8; 3]> = (0..1200).map(|i| detail[i * n / 1200]).collect();
            pioneer_usb_writer::models::ColorWaveform { detail, overview }
        });

    pioneer_usb_writer::models::AnalysisResult {
        bpm,
        key,
        beat_grid: pioneer_usb_writer::models::BeatGrid { beats },
        waveform: pioneer_usb_writer::models::WaveformPreview { data: waveform_data },
        cue_points: Vec::new(),
        color_waveform,
    }
}
