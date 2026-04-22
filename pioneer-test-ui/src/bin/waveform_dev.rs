/// waveform-dev — standalone waveform dev harness.
///
/// Usage:
///   cargo run --bin waveform-dev -- <audio-file> [port]
///
/// Analyzes the given audio file using the Python fourfour_analysis CLI,
/// writes waveform data to ui/waveform/data.json, starts a local HTTP server,
/// and opens dev.html in the browser. Press Enter to re-analyze, or type a
/// new file path + Enter to switch tracks. Ctrl-C to quit.
use anyhow::Context as _;
use std::io::{self, BufRead, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use pioneer_usb_writer::models::AnalysisResult;
use pioneer_usb_writer::reader::{anlz as anlz_reader, masterdb};
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
    if let Err(e) = analyze_and_write(&audio_path, &data_path, &serve_dir) {
        eprintln!("Analysis failed: {e}");
        std::process::exit(1);
    }
    if let Err(e) = write_rekordbox_json(&audio_path, &serve_dir) {
        eprintln!("Rekordbox lookup: {e}");
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

        match analyze_and_write(&audio_path, &data_path, &serve_dir) {
            Ok(_) => {
                if let Err(e) = write_rekordbox_json(&audio_path, &serve_dir) {
                    eprintln!("  (Rekordbox: {e})");
                }
                println!("done — refresh browser tab.");
            }
            Err(e) => eprintln!("failed: {e}"),
        }
    }

    let _ = server.kill();
}

// ── Analysis ─────────────────────────────────────────────────────────────────

fn resolve_python() -> String {
    // Look for venv Python relative to the workspace root (two levels up from this binary's source)
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

fn analyze_and_write(audio_path: &Path, data_path: &Path, serve_dir: &Path) -> anyhow::Result<()> {
    let python = resolve_python();
    let output = Command::new(&python)
        .args([
            "-m",
            "fourfour_analysis",
            "waveform-compare",
            audio_path.to_str().ok_or_else(|| anyhow::anyhow!("non-UTF8 path"))?,
            "--json",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Python waveform-compare failed: {stderr}");
    }

    // Symlink the audio file into serve_dir so the browser can fetch it
    let ext = audio_path.extension().and_then(|e| e.to_str()).unwrap_or("audio");
    let link_name = format!("current_track.{ext}");
    let link_path = serve_dir.join(&link_name);
    let _ = std::fs::remove_file(&link_path);
    #[cfg(unix)]
    std::os::unix::fs::symlink(audio_path.canonicalize().unwrap_or(audio_path.to_path_buf()), &link_path)
        .ok();

    let track_url = format!("./{link_name}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let mut value = compare_result_to_display_json(&result);
    value["track_url"] = serde_json::Value::String(track_url);
    std::fs::write(data_path, serde_json::to_string(&value)?)?;
    Ok(())
}

/// Convert a {min_val, max_val, r, g, b} column list to [{amp, r, g, b}].
fn cols_to_display(cols: &[serde_json::Value]) -> Vec<serde_json::Value> {
    cols.iter().map(|c| {
        let r = c.get("r").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let g = c.get("g").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b = c.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let min_val = c.get("min_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let max_val = c.get("max_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let amp = ((max_val - min_val) / 2.0).min(1.0).max(0.0);
        json!({ "amp": amp, "r": r as u8, "g": g as u8, "b": b as u8 })
    }).collect()
}

/// Convert the `waveform-compare` JSON output to the shape expected by dev.html.
fn compare_result_to_display_json(result: &serde_json::Value) -> serde_json::Value {
    let bpm = result.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let key = result.get("key").and_then(|v| v.as_str()).unwrap_or("");

    // beats [{time_seconds, bar_position}] → [{time_ms, bar_position}]
    let beats: Vec<serde_json::Value> = result
        .get("beats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|b| {
                    let time_secs = b.get("time_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let bar_pos = b.get("bar_position").and_then(|v| v.as_u64()).unwrap_or(1);
                    json!({ "time_ms": (time_secs * 1000.0) as u64, "bar_position": bar_pos })
                })
                .collect()
        })
        .unwrap_or_default();

    // Duration from lexicon waveform column count (80 samples @ 12 kHz = 6.67 ms/col)
    let beat_interval_ms = if bpm > 0.0 { (60_000.0 / bpm) as u64 } else { 500 };
    let peak_count = result
        .get("waveform_peaks")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let duration_ms = if peak_count > 0 {
        (peak_count as f64 * 80.0 / 12_000.0 * 1_000.0) as u64
    } else {
        result
            .get("beats")
            .and_then(|v| v.as_array())
            .and_then(|a| a.last())
            .and_then(|b| b.get("time_seconds"))
            .and_then(|v| v.as_f64())
            .map(|s| (s * 1_000.0) as u64)
            .unwrap_or(0)
            + beat_interval_ms * 4
    };

    // 400-byte waveform preview from waveform_peaks
    let waveform_preview: Vec<u8> = result
        .get("waveform_peaks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let n = arr.len();
            (0..400usize)
                .map(|i| {
                    let src = &arr[if n > 0 { i * n / 400 } else { 0 }];
                    let max_val = src.get("max_val").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let height = (max_val.abs().min(1.0) * 31.0) as u8;
                    height << 3
                })
                .collect()
        })
        .unwrap_or_else(|| vec![0u8; 400]);

    // Convert each backend's column list to display format
    let waveforms_out: serde_json::Map<String, serde_json::Value> = result
        .get("waveforms")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(name, cols)| {
                    cols.as_array().map(|arr| {
                        (name.clone(), serde_json::Value::Array(cols_to_display(arr)))
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let waveform_fft_bands = result.get("waveform_fft_bands").cloned().unwrap_or(json!([]));

    json!({
        "bpm": bpm,
        "key": key,
        "beats": beats,
        "duration_ms": duration_ms,
        "waveform_preview": waveform_preview,
        "waveform_fft_bands": waveform_fft_bands,
        "waveforms": waveforms_out,
    })
}

// ── Rekordbox ANLZ ───────────────────────────────────────────────────────────

fn write_rekordbox_json(audio_path: &Path, serve_dir: &Path) -> anyhow::Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let master_db = PathBuf::from(home).join("Library/Pioneer/rekordbox/master.db");

    let canonical = audio_path
        .canonicalize()
        .with_context(|| format!("Cannot resolve path: {}", audio_path.display()))?;
    let anlz_path = masterdb::find_anlz_path(&master_db, &canonical)
        .with_context(|| format!("Cannot find ANLZ for {}", canonical.display()))?;

    let result = anlz_reader::read_anlz(&anlz_path)?;
    let value = anlz_result_to_display_json(&result);

    let out_path = serve_dir.join("rekordbox.json");
    std::fs::write(out_path, serde_json::to_string(&value)?)?;
    eprintln!("  Rekordbox ANLZ: {}", anlz_path.display());
    Ok(())
}

/// Decode a PWV3/PWV4 RGB triplet to {amp, r, g, b} for WaveformDisplay.
///
/// Pioneer encodes: dominant band = height×4 (max 124), others = height (max 31).
/// We normalise so dominant = 255 and set amp = max_channel / 124.
fn decode_pwv_rgb(rgb: &[u8; 3]) -> serde_json::Value {
    let max_ch = rgb[0].max(rgb[1]).max(rgb[2]).max(1) as f64;
    let amp = (max_ch / 124.0).min(1.0);
    let scale = 255.0 / max_ch;
    let r = (rgb[0] as f64 * scale).min(255.0) as u8;
    let g = (rgb[1] as f64 * scale).min(255.0) as u8;
    let b = (rgb[2] as f64 * scale).min(255.0) as u8;
    json!({ "amp": amp, "r": r, "g": g, "b": b })
}

/// Convert an [`AnalysisResult`] read from Rekordbox ANLZ into the display JSON
/// format expected by dev.html. Includes both detail (~150 cols/sec) and overview
/// (1200-entry fixed) waveforms.
fn anlz_result_to_display_json(result: &AnalysisResult) -> serde_json::Value {
    let bpm = result.bpm;
    let key = result.key.as_str();

    // PWV3 color waveform detail → [{amp, r, g, b}]
    let waveform_detail: Vec<serde_json::Value> = result
        .color_waveform
        .as_ref()
        .map(|cw| cw.detail.iter().map(decode_pwv_rgb).collect())
        .unwrap_or_default();

    // PWV4 color waveform overview → [{amp, r, g, b}] (1200 fixed entries)
    let waveform_overview: Vec<serde_json::Value> = result
        .color_waveform
        .as_ref()
        .map(|cw| cw.overview.iter().map(decode_pwv_rgb).collect())
        .unwrap_or_default();

    // Beat grid → [{time_ms, bar_position}]
    let beats: Vec<serde_json::Value> = result
        .beat_grid
        .beats
        .iter()
        .map(|b| json!({ "time_ms": b.time_ms as u64, "bar_position": b.bar_position as u64 }))
        .collect();

    // Duration from detail waveform length (150 cols/sec)
    let beat_interval_ms = if bpm > 0.0 { (60_000.0 / bpm) as u64 } else { 500 };
    let duration_ms = if !waveform_detail.is_empty() {
        (waveform_detail.len() as f64 / 150.0 * 1000.0) as u64
    } else if let Some(last) = result.beat_grid.beats.last() {
        last.time_ms as u64 + beat_interval_ms * 4
    } else {
        0
    };

    let waveform_preview: Vec<u8> = result.waveform.data.to_vec();

    json!({
        "bpm": bpm,
        "key": key,
        "beats": beats,
        "duration_ms": duration_ms,
        "waveform_preview": waveform_preview,
        "waveform_detail": waveform_detail,
        "waveform_overview": waveform_overview,
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
