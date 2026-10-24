use std::path::Path;

use pioneer_usb_writer::models;

/// Resolve the Python venv binary. Uses CARGO_MANIFEST_DIR (compile-time) so it
/// works regardless of the process working directory at runtime.
pub(crate) fn resolve_python() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let venv_python = Path::new(manifest_dir)
        .parent() // workspace root (one level up from pioneer-test-ui)
        .unwrap_or(Path::new("."))
        .join("analysis/.venv/bin/python");
    if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        "python3".to_string()
    }
}

/// Unwrap analyzer stdout JSON into the per-track result object.
///
/// Accepts legacy `[{...}]` batches, current `{"deeprhythm_essentia": {...}}`
/// backend-keyed output from `python -m fourfour_analysis analyze`, and a flat
/// `{bpm, key, ...}` object.
pub(crate) fn parse_analyzer_stdout(stdout: &[u8]) -> Result<serde_json::Value, String> {
    let json: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|e| format!("Bad JSON ({} bytes): {e}", stdout.len()))?;

    if let Some(arr) = json.as_array() {
        return arr
            .first()
            .cloned()
            .ok_or_else(|| "Analyzer returned empty array".to_string());
    }

    if let Some(obj) = json.as_object() {
        if obj.contains_key("bpm") || obj.contains_key("waveform_preview") {
            return Ok(json);
        }
        if let Some(inner) = obj.values().find(|v| v.is_object()) {
            return Ok(inner.clone());
        }
    }

    Err("Analyzer JSON has no recognizable result object".to_string())
}

/// Convert a Python fourfour_analysis JSON result to `models::AnalysisResult`.
pub(crate) fn python_result_to_analysis(json: &serde_json::Value) -> models::AnalysisResult {
    let bpm = json.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let key = json.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let energy = json.get("energy").and_then(|v| {
        v.as_u64()
            .map(|s| s as u8)
            .or_else(|| v.get("score").and_then(|s| s.as_u64()).map(|s| s as u8))
    });

    // Mono waveform preview (400 bytes)
    let mut waveform_data = [0u8; 400];
    if let Some(arr) = json.get("waveform_preview").and_then(|v| v.as_array()) {
        for (i, val) in arr.iter().enumerate().take(400) {
            waveform_data[i] = val.as_u64().unwrap_or(0) as u8;
        }
    }

    // Parse 3-band color waveform. The calibrated backend emits objects
    // `{r,g,b}` (0–127) under `waveform_colors`/`waveform_overview`; older
    // output used `[low,mid,high]` arrays under `pioneer_3band_*`. Accept both.
    let color_waveform = {
        let parse_3band = |keys: &[&str]| -> Vec<[u8; 3]> {
            keys.iter()
                .find_map(|k| json.get(*k).and_then(|v| v.as_array()))
                .map(|arr| {
                    arr.iter()
                        .map(|entry| {
                            if let Some(obj) = entry.as_object() {
                                let ch = |name: &str| {
                                    obj.get(name).and_then(|v| v.as_u64()).unwrap_or(0) as u8
                                };
                                [ch("r"), ch("g"), ch("b")]
                            } else if let Some(a) = entry.as_array() {
                                let ch = |i: usize| {
                                    a.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u8
                                };
                                [ch(0), ch(1), ch(2)]
                            } else {
                                [0, 0, 0]
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let detail = parse_3band(&["waveform_colors", "pioneer_3band_detail"]);
        let overview = parse_3band(&["waveform_overview", "pioneer_3band_overview"]);

        if !detail.is_empty() {
            Some(models::ColorWaveform { detail, overview })
        } else {
            None
        }
    };

    // Parse beat grid from Python BeatPosition entries
    let mut beats: Vec<models::Beat> = json
        .get("beats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let time_s = entry.get("time_seconds")?.as_f64()?;
                    let bar_pos = entry.get("bar_position")?.as_u64()? as u8;
                    Some(models::Beat {
                        bar_position: bar_pos,
                        time_ms: (time_s * 1000.0).round() as u32,
                        tempo: 0,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Infer tempo from consecutive beat intervals (stored as BPM * 100)
    if beats.len() > 1 {
        for i in 1..beats.len() {
            let delta_ms = beats[i].time_ms.saturating_sub(beats[i - 1].time_ms);
            if delta_ms > 0 {
                beats[i].tempo = (60_000.0 / delta_ms as f64 * 100.0).round() as u32;
            }
        }
        beats[0].tempo = beats[1].tempo;
    } else if !beats.is_empty() && bpm > 0.0 {
        beats[0].tempo = (bpm * 100.0).round() as u32;
    }

    // Parse cue points — auto-detected cues are stored as memory cues (hot_cue_number = 0)
    let cue_points: Vec<models::CuePoint> = json
        .get("cue_points")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let time_s = entry.get("time_seconds")?.as_f64()?;
                    let loop_end = entry
                        .get("loop_end_seconds")
                        .and_then(|v| v.as_f64())
                        .filter(|&t| t > 0.0);
                    Some(models::CuePoint {
                        hot_cue_number: 0,
                        time_ms: (time_s * 1000.0).round() as u32,
                        loop_time_ms: loop_end.map(|t| (t * 1000.0).round() as u32),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    models::AnalysisResult {
        bpm,
        key,
        energy,
        beat_grid: models::BeatGrid { beats },
        waveform: models::WaveformPreview { data: waveform_data },
        cue_points,
        color_waveform,
    }
}

/// Run the Python analyzer for one track.
pub(crate) async fn analyze_one_track(
    python_cmd: &str,
    _track_id: i64,
    source_path: &Path,
) -> Result<(models::AnalysisResult, String), String> {
    let path_str = source_path.to_string_lossy().to_string();
    let t0 = std::time::Instant::now();

    let child = tokio::process::Command::new(python_cmd)
        .args([
            "-m",
            "fourfour_analysis",
            "analyze",
            &path_str,
            "--backend",
            "deeprhythm_essentia",
            "--json",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn analyzer: {e}"))?;

    let output = tokio::time::timeout(std::time::Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| "TIMEOUT after 120s".to_string())?
        .map_err(|e| format!("wait_with_output failed: {e}"))?;

    let elapsed = t0.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout_len = output.stdout.len();

    if !output.status.success() {
        return Err(format!(
            "exit={} stderr={}",
            output.status.code().unwrap_or(-1),
            &stderr[..stderr.len().min(500)]
        ));
    }

    if !stderr.trim().is_empty() {
        eprintln!("[analyzer stderr] {}", &stderr[..stderr.len().min(300)]);
    }

    let result_json = parse_analyzer_stdout(&output.stdout)?;
    let analysis = python_result_to_analysis(&result_json);
    Ok((
        analysis,
        format!("{:.1}s {}bytes", elapsed.as_secs_f64(), stdout_len),
    ))
}
