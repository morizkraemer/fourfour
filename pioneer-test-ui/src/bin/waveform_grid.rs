/// waveform-grid — Write multiple hypothesis variants of tracks to USB.
///
/// Usage:
///   cargo run --bin waveform_grid -- <audio-file-or-dir> <usb-path>
///
/// If a directory is given, all audio files inside are processed.
/// Generates all hypothesis waveform variants per track and writes each as a
/// separate track on the USB. Also includes production and Rekordbox refs.
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

use pioneer_usb_writer::models::{AnalysisResult, ColorWaveform};
use pioneer_usb_writer::reader::{anlz as anlz_reader, masterdb};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: waveform_grid <audio-file-or-dir> <usb-path>");
        eprintln!("Example: cargo run --bin waveform_grid -- ~/Music/Hoodlum /Volumes/ORANGE");
        std::process::exit(1);
    }

    let source_path = PathBuf::from(&args[0]);
    let usb_path = PathBuf::from(&args[1]);

    // Collect audio files
    let audio_files = if source_path.is_dir() {
        collect_audio_files(&source_path)
    } else {
        vec![source_path.clone()]
    };

    if audio_files.is_empty() {
        eprintln!("No audio files found at {}", source_path.display());
        std::process::exit(1);
    }

    println!("Found {} audio file(s)", audio_files.len());

    if let Err(e) = run(&audio_files, &usb_path) {
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}

fn collect_audio_files(dir: &Path) -> Vec<PathBuf> {
    let exts: std::collections::HashSet<&str> =
        ["flac", "wav", "mp3", "m4a", "aac", "aiff", "ogg"]
            .iter()
            .cloned()
            .collect();

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if exts.contains(ext.to_lowercase().as_str()) {
                        files.push(path);
                    }
                }
            }
        }
    }
    files.sort();
    files
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

fn run(audio_files: &[PathBuf], usb_path: &Path) -> Result<()> {
    // 1. Wipe USB
    println!("Wiping Pioneer data from {}…", usb_path.display());
    let pioneer = usb_path.join("PIONEER");
    let contents = usb_path.join("Contents");
    if pioneer.exists() { std::fs::remove_dir_all(&pioneer)?; }
    if contents.exists() { std::fs::remove_dir_all(&contents)?; }

    let home = std::env::var("HOME").context("HOME not set")?;
    let master_db = PathBuf::from(&home).join("Library/Pioneer/rekordbox/master.db");

    let mut all_tracks: Vec<pioneer_usb_writer::models::Track> = Vec::new();
    let mut all_analyses: Vec<AnalysisResult> = Vec::new();
    let mut next_id: u32 = 1;

    for audio in audio_files {
        let audio = audio.canonicalize()
            .with_context(|| format!("Cannot resolve: {}", audio.display()))?;

        println!("\n=== {} ===", audio.file_name().unwrap_or_default().to_string_lossy());

        // 2. Scan
        let base_tracks = pioneer_usb_writer::scanner::scan_files(&[audio.clone()])?;
        if base_tracks.is_empty() {
            println!("  Skipped — no metadata");
            continue;
        }
        let base_track = &base_tracks[0];

        // 3. Run Python for hypotheses
        println!("  Generating hypotheses…");
        let python = resolve_python();
        let py_output = std::process::Command::new(&python)
            .args([
                "-m",
                "fourfour_analysis",
                "waveform-compare",
                audio.to_str().unwrap(),
                "--hypotheses",
                "--json",
            ])
            .current_dir(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("analysis/src"),
            )
            .output()
            .with_context(|| format!("Failed to run Python: {python}"))?;

        if !py_output.status.success() {
            let stderr = String::from_utf8_lossy(&py_output.stderr);
            anyhow::bail!("Python failed:\n{stderr}");
        }

        let json: serde_json::Value = serde_json::from_slice(&py_output.stdout)?;
        let bpm = json.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let key = json.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let beats_json = json.get("beats").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let beats: Vec<pioneer_usb_writer::models::Beat> = beats_json
            .iter()
            .map(|b| {
                let bar_pos = b.get("bar_position").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                let time_ms = b.get("time_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0) * 1000.0;
                pioneer_usb_writer::models::Beat {
                    bar_position: bar_pos,
                    time_ms: time_ms as u32,
                    tempo: (bpm * 100.0) as u32,
                }
            })
            .collect();

        let mut waveform_data = [0u8; 400];
        if let Some(arr) = json.get("waveform_peaks").and_then(|v| v.as_array()) {
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

        let waveforms = json
            .get("waveforms")
            .and_then(|v| v.as_object())
            .context("No waveforms in JSON output")?;

        let ext = audio.extension().and_then(|e| e.to_str()).unwrap_or("flac");

        // 4. Build tracks for each hypothesis variant
        for (name, cols) in waveforms.iter() {
            let cols_arr = cols.as_array().context("waveform cols not array")?;
            let detail: Vec<[u8; 3]> = cols_arr
                .iter()
                .map(|c| {
                    let r = c.get("r").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    let g = c.get("g").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    let b = c.get("b").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    [r, g, b]
                })
                .collect();
            let n = detail.len();
            let overview: Vec<[u8; 3]> = (0..1200).map(|i| detail[i * n / 1200]).collect();

            let analysis = AnalysisResult {
                bpm,
                key: key.clone(),
                beat_grid: pioneer_usb_writer::models::BeatGrid { beats: beats.clone() },
                waveform: pioneer_usb_writer::models::WaveformPreview { data: waveform_data },
                cue_points: Vec::new(),
                color_waveform: Some(ColorWaveform { detail, overview }),
            };

            let mut track = base_track.clone();
            track.id = next_id;
            next_id += 1;
            track.title = format!("{} [{}]", base_track.title, name);
            let filename = format!("{} [{}].{}", base_track.title, name, ext);
            track.usb_path = track
                .usb_path
                .rsplit_once('/')
                .map(|(dir, _)| format!("{}/{}", dir, filename))
                .unwrap_or(filename);
            track.source_path = base_track.source_path.clone();

            all_tracks.push(track);
            all_analyses.push(analysis);
        }

        // 5. Add Rekordbox reference
        println!("  Reading Rekordbox ANLZ…");
        match masterdb::find_anlz_path(&master_db, &audio) {
            Ok(anlz_path) => {
                let rb_analysis = anlz_reader::read_anlz(&anlz_path)?;
                let mut rb_track = base_track.clone();
                rb_track.id = next_id;
                next_id += 1;
                rb_track.title = format!("{} [Rekordbox]", base_track.title);
                let rb_filename = format!("{} [Rekordbox].{}", base_track.title, ext);
                rb_track.usb_path = rb_track
                    .usb_path
                    .rsplit_once('/')
                    .map(|(dir, _)| format!("{}/{}", dir, rb_filename))
                    .unwrap_or(rb_filename);
                rb_track.source_path = base_track.source_path.clone();

                all_tracks.push(rb_track);
                all_analyses.push(rb_analysis);
            }
            Err(e) => {
                println!("  (No Rekordbox reference: {e})");
            }
        }
    }

    println!("\nWriting {} tracks to {}…", all_tracks.len(), usb_path.display());
    pioneer_usb_writer::writer::filesystem::write_usb(
        usb_path,
        &all_tracks,
        &all_analyses,
        &[],
    )?;

    println!("\nDone! Eject {} and test on CDJ.", usb_path.display());
    Ok(())
}
