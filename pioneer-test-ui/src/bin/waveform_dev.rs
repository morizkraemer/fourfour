/// waveform-dev — standalone waveform dev harness.
///
/// Usage:
///   cargo run --bin waveform-dev -- <audio-file> [port]
///
/// Analyzes the given audio file, writes waveform data to
/// ui/waveform/data.json, starts a local HTTP server, and opens
/// dev.html in the browser. Press Enter to re-analyze, or type a
/// new file path + Enter to switch tracks. Ctrl-C to quit.
use std::io::{self, BufRead, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use pioneer_test_ui::analyzer::analyze_track;
use pioneer_usb_writer::models;
use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: waveform-dev <audio-file> [port]");
        std::process::exit(1);
    }

    let mut audio_path = PathBuf::from(&args[1]);
    let port: u16 = args.get(2).and_then(|p| p.parse().ok()).unwrap_or(8080);

    // Locate ui/waveform/ from the current working directory
    let cwd = std::env::current_dir().expect("Failed to get cwd");
    let serve_dir = cwd.join("ui/waveform");
    if !serve_dir.exists() {
        eprintln!(
            "Error: ui/waveform/ not found in {}. Run from the repo root.",
            cwd.display()
        );
        std::process::exit(1);
    }

    let data_path = serve_dir.join("data.json");

    // Initial analysis
    if let Err(e) = analyze_and_write(&audio_path, &data_path) {
        eprintln!("Analysis failed: {e}");
        std::process::exit(1);
    }

    // Start HTTP server (Python, quiet)
    let mut server = start_server(&serve_dir, port);
    std::thread::sleep(std::time::Duration::from_millis(200)); // let server start

    // Open browser
    let _ = Command::new("open")
        .arg(format!("http://localhost:{port}/dev.html"))
        .spawn();

    println!("Serving http://localhost:{port}/dev.html");
    println!("  Enter         — re-analyze current track");
    println!("  <file path>   — analyze a different track");
    println!("  Ctrl-C        — quit");
    println!();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim();
        if !line.is_empty() {
            let candidate = PathBuf::from(line);
            if candidate.exists() {
                audio_path = candidate;
            } else {
                eprintln!("File not found: {line}");
                continue;
            }
        }

        print!("Analyzing {}…  ", audio_path.display());
        io::stdout().flush().ok();

        match analyze_and_write(&audio_path, &data_path) {
            Ok(_) => println!("done — refresh browser tab."),
            Err(e) => eprintln!("failed: {e}"),
        }
    }

    let _ = server.kill();
}

// ── Analysis ─────────────────────────────────────────────────────────────────

fn analyze_and_write(audio_path: &Path, data_path: &Path) -> anyhow::Result<()> {
    let result = analyze_track(audio_path)?;
    let duration_ms = estimate_duration_ms(&result);
    let value = to_json(&result, duration_ms);
    std::fs::write(data_path, serde_json::to_string(&value)?)?;
    Ok(())
}

/// Estimate duration from the beat grid last beat + one bar.
fn estimate_duration_ms(result: &models::AnalysisResult) -> u64 {
    let last_beat_ms = result
        .beat_grid
        .beats
        .last()
        .map(|b| b.time_ms as u64)
        .unwrap_or(0);
    let beat_interval_ms = if result.bpm > 0.0 {
        (60_000.0 / result.bpm) as u64
    } else {
        500
    };
    last_beat_ms + beat_interval_ms * 4
}

/// Convert AnalysisResult to the JSON shape expected by WaveformDisplay.setData().
/// Mirrors the `get_analysis_data` Tauri command in main.rs.
fn to_json(result: &models::AnalysisResult, duration_ms: u64) -> serde_json::Value {
    let waveform_color: Vec<serde_json::Value> = result
        .color_waveform
        .as_ref()
        .map(|cw| {
            let source = if !cw.detail.is_empty() {
                &cw.detail
            } else {
                &cw.overview
            };
            source
                .iter()
                .map(|[low, mid, high]| {
                    let max_val = (*low).max(*mid).max(*high) as f64;
                    let scale = if max_val > 0.0 { 1.0 / max_val } else { 0.0 };
                    let amp = (max_val / 255.0).min(1.0);
                    json!({
                        "amp": amp,
                        "r": (*low as f64 * scale * 255.0) as u8,
                        "g": (*mid as f64 * scale * 255.0) as u8,
                        "b": (*high as f64 * scale * 255.0) as u8,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let beats: Vec<serde_json::Value> = result
        .beat_grid
        .beats
        .iter()
        .map(|b| {
            json!({
                "time_ms": b.time_ms,
                "bar_position": b.bar_position,
            })
        })
        .collect();

    json!({
        "waveform_preview": result.waveform.data.to_vec(),
        "waveform_color": waveform_color,
        "beats": beats,
        "duration_ms": duration_ms,
        "bpm": result.bpm,
        "key": result.key,
    })
}

// ── HTTP server ───────────────────────────────────────────────────────────────

fn start_server(serve_dir: &Path, port: u16) -> Child {
    Command::new("python3")
        .args([
            "-m",
            "http.server",
            &port.to_string(),
            "-d",
            serve_dir.to_str().expect("non-UTF8 path"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start python3 HTTP server")
}
