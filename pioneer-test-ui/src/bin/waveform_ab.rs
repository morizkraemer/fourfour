/// waveform-ab — Write two copies of a track to USB: ours vs Rekordbox waveform.
///
/// Usage:
///   cargo run --bin waveform_ab -- <audio-file> <usb-path>
///
/// Writes the same audio file twice:
///   1. "Title - Artist"       → our filter bank waveform
///   2. "Title - Artist (RB)"  → Rekordbox's original waveform data
///
/// This lets you A/B compare on the CDJ by flipping between the two tracks.
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

use pioneer_usb_writer::models::{AnalysisResult, ColorWaveform};
use pioneer_usb_writer::reader::{anlz as anlz_reader, masterdb};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: waveform_ab <audio-file> <usb-path>");
        eprintln!("Example: cargo run --bin waveform_ab -- Hoodlum/Hoodlum\\ -\\ Traumer.flac /Volumes/ORANGE");
        std::process::exit(1);
    }

    let audio_path = PathBuf::from(&args[0]);
    let usb_path = PathBuf::from(&args[1]);

    if let Err(e) = run(&audio_path, &usb_path) {
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}

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

fn run(audio_path: &Path, usb_path: &Path) -> Result<()> {
    let audio = audio_path
        .canonicalize()
        .with_context(|| format!("Cannot resolve: {}", audio_path.display()))?;

    // 1. Wipe USB
    println!("Wiping Pioneer data from {}…", usb_path.display());
    let pioneer = usb_path.join("PIONEER");
    let contents = usb_path.join("Contents");
    if pioneer.exists() { std::fs::remove_dir_all(&pioneer)?; }
    if contents.exists() { std::fs::remove_dir_all(&contents)?; }

    // 2. Scan the audio file — we'll use this as the base for both tracks
    println!("Scanning {}…", audio.display());
    let tracks = pioneer_usb_writer::scanner::scan_files(&[audio.clone()])?;
    if tracks.is_empty() {
        anyhow::bail!("No tracks found");
    }

    // 3. Analyze with our Python backend
    println!("Analyzing with our backend…");
    let our_analysis = run_python_analysis(&audio)?;
    println!(
        "  BPM: {:.1}, Key: {}, Beats: {}, Color: {}",
        our_analysis.bpm,
        our_analysis.key,
        our_analysis.beat_grid.beats.len(),
        if our_analysis.color_waveform.is_some() { "yes" } else { "no" },
    );

    // 4. Read Rekordbox's ANLZ data
    println!("Reading Rekordbox ANLZ…");
    let home = std::env::var("HOME").context("HOME not set")?;
    let master_db = PathBuf::from(&home).join("Library/Pioneer/rekordbox/master.db");
    let anlz_path = masterdb::find_anlz_path(&master_db, &audio)?;
    let rb_analysis = anlz_reader::read_anlz(&anlz_path)?;
    println!(
        "  Rekordbox — BPM: {:.1}, Beats: {}, Color: {}",
        rb_analysis.bpm,
        rb_analysis.beat_grid.beats.len(),
        if rb_analysis.color_waveform.is_some() { "yes" } else { "no" },
    );

    // 5. Create two tracks: ours (id=1) and RB (id=2)
    let mut track_ours = tracks[0].clone();
    track_ours.id = 1;

    let mut track_rb = tracks[0].clone();
    track_rb.id = 2;
    track_rb.title = format!("{} (RB)", track_rb.title);
    // Adjust the USB path so they don't collide
    let rb_filename = format!(
        "{} (RB).{}",
        audio.file_stem().unwrap().to_string_lossy(),
        audio.extension().unwrap().to_string_lossy()
    );
    track_rb.usb_path = track_rb
        .usb_path
        .rsplit_once('/')
        .map(|(dir, _)| format!("{}/{}", dir, rb_filename))
        .unwrap_or(rb_filename);
    // Point source_path to the same file — write_usb will copy it under the new name
    track_rb.source_path = track_ours.source_path.clone();

    println!(
        "\nWriting to {}…",
        usb_path.display()
    );
    println!("  Track 1: {} (our waveform)", track_ours.title);
    println!("  Track 2: {} (Rekordbox waveform)", track_rb.title);

    // 6. Write both to USB
    pioneer_usb_writer::writer::filesystem::write_usb(
        usb_path,
        &[track_ours, track_rb],
        &[our_analysis, rb_analysis],
        &[],
    )?;

    println!("\nDone! Eject {} and test on CDJ.", usb_path.display());
    Ok(())
}

fn run_python_analysis(audio: &Path) -> Result<AnalysisResult> {
    let python = resolve_python();
    let output = std::process::Command::new(&python)
        .args([
            "-m",
            "fourfour_analysis",
            "analyze",
            audio.to_str().unwrap(),
            "--json",
            "--backend",
            "final_stack",
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

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // Re-use the same parser as pioneer-cli
    let result = json
        .as_object()
        .and_then(|obj| obj.values().next())
        .filter(|v| v.is_object())
        .unwrap_or(&json);

    let bpm = result.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let key = result.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let beats: Vec<pioneer_usb_writer::models::Beat> = result
        .get("beats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let mut out = Vec::new();
            for b in arr {
                let bar_pos = b.get("bar_position").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                if let Some(time_secs) = b.get("time_seconds").and_then(|v| v.as_f64()) {
                    out.push(pioneer_usb_writer::models::Beat {
                        bar_position: bar_pos,
                        time_ms: (time_secs * 1000.0) as u32,
                        tempo: (bpm * 100.0) as u32,
                    });
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
            ColorWaveform { detail, overview }
        });

    Ok(AnalysisResult {
        bpm,
        key,
        beat_grid: pioneer_usb_writer::models::BeatGrid { beats },
        waveform: pioneer_usb_writer::models::WaveformPreview { data: waveform_data },
        cue_points: Vec::new(),
        color_waveform,
    })
}
