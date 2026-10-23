#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis_payload;
mod dto;
mod playback;
mod track_infos;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;
use pioneer_usb_writer::scanner;

use track_infos::build_track_infos;
use dto::{LoadedState, PlaylistInput, ProgressPayload, TrackInfo};
use playback::{
    playback_configure, playback_get_config, playback_pause, playback_play, playback_resume,
    playback_get_output_device, playback_list_output_devices, playback_seek,
    playback_set_output_device, playback_set_position, playback_stop, PlaybackEngine,
};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

type SharedLibrary = Arc<Mutex<LocalLibrary>>;

type AnalysisWorkItem = (i64, PathBuf);
type SharedAnalysisQueue = Arc<Mutex<VecDeque<AnalysisWorkItem>>>;

/// Live analysis run state — exposes the work queue for reorder / enqueue while running.
#[derive(Clone)]
struct AnalysisCoordinator {
    active_queue: Arc<Mutex<Option<SharedAnalysisQueue>>>,
    total: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
}

impl Default for AnalysisCoordinator {
    fn default() -> Self {
        Self {
            active_queue: Arc::new(Mutex::new(None)),
            total: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AnalysisCoordinator {
    fn is_active(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Returns true when this caller owns the run (false = work was merged into an active run).
    fn try_begin(&self) -> bool {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    fn register_run(&self, queue: SharedAnalysisQueue, total: u32) {
        *self.active_queue.lock().expect("analysis coordinator lock") = Some(queue);
        self.total.store(total, Ordering::Relaxed);
    }

    fn end_run(&self) {
        *self.active_queue.lock().expect("analysis coordinator lock") = None;
        self.total.store(0, Ordering::Relaxed);
        self.running.store(false, Ordering::Release);
    }

    fn with_queue<R>(&self, f: impl FnOnce(&mut VecDeque<AnalysisWorkItem>) -> R) -> Option<R> {
        let guard = self.active_queue.lock().ok()?;
        let q = guard.as_ref()?;
        let mut deque = q.lock().ok()?;
        Some(f(&mut deque))
    }

    fn bump_total(&self, delta: u32) {
        if delta > 0 {
            self.total.fetch_add(delta, Ordering::Relaxed);
        }
    }

    fn total(&self) -> u32 {
        self.total.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Debug logging — emits events to the frontend debug panel
// ---------------------------------------------------------------------------

fn debug_log(app: &AppHandle, level: &str, message: &str) {
    app.emit("debug-log", serde_json::json!({ "level": level, "message": message })).ok();
}

fn debug_info(app: &AppHandle, msg: &str)  { debug_log(app, "info", msg); }
fn debug_ok(app: &AppHandle, msg: &str)    { debug_log(app, "ok", msg); }
fn debug_err(app: &AppHandle, msg: &str)   { debug_log(app, "err", msg); }
fn debug_dim(app: &AppHandle, msg: &str)   { debug_log(app, "dim", msg); }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Scan a directory for audio files and add them to the library.
#[tauri::command]
fn scan_directory(
    path: String,
    state: State<'_, SharedLibrary>,
) -> Result<Vec<TrackInfo>, String> {
    let dir = Path::new(&path);
    let scanned = scanner::scan_directory(dir).map_err(|e| e.to_string())?;

    let lib = state.lock().map_err(|e| e.to_string())?;

    let mut added = Vec::new();
    for track in &scanned {
        // Skip tracks already in the library
        if lib
            .track_exists_by_path(&track.source_path)
            .map_err(|e| e.to_string())?
        {
            continue;
        }
        let id = lib.add_track(track).map_err(|e| e.to_string())?;
        let has_artwork = track.artwork.is_some();
        added.push(TrackInfo {
            id: id as u32,
            source_path: track.source_path.to_string_lossy().to_string(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            genre: track.genre.clone(),
            tempo: track.tempo,
            key: track.key.clone(),
            duration_secs: track.duration_secs,
            bitrate: track.bitrate,
            file_size: track.file_size,
            has_artwork,
            has_cues: false,
            has_analysis: false,
            waveform_preview: None,
        });
    }

    Ok(added)
}

/// Scan specific files and/or folders and add them to the library.
/// Directories are recursed into via scan_directory; files are scanned directly.
#[tauri::command]
fn scan_files(
    paths: Vec<String>,
    state: State<'_, SharedLibrary>,
) -> Result<Vec<TrackInfo>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dir_tracks: Vec<models::Track> = Vec::new();

    for p in &paths {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            let found = scanner::scan_directory(&pb).map_err(|e| e.to_string())?;
            dir_tracks.extend(found);
        } else {
            files.push(pb);
        }
    }

    let mut scanned = scanner::scan_files(&files).map_err(|e| e.to_string())?;
    scanned.extend(dir_tracks);

    let lib = state.lock().map_err(|e| e.to_string())?;

    let mut added = Vec::new();
    for track in &scanned {
        if lib
            .track_exists_by_path(&track.source_path)
            .map_err(|e| e.to_string())?
        {
            continue;
        }
        let id = lib.add_track(track).map_err(|e| e.to_string())?;
        let has_artwork = track.artwork.is_some();
        added.push(TrackInfo {
            id: id as u32,
            source_path: track.source_path.to_string_lossy().to_string(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            genre: track.genre.clone(),
            tempo: track.tempo,
            key: track.key.clone(),
            duration_secs: track.duration_secs,
            bitrate: track.bitrate,
            file_size: track.file_size,
            has_artwork,
            has_cues: false,
            has_analysis: false,
            waveform_preview: None,
        });
    }

    Ok(added)
}

/// Resolve the Python venv binary. Uses CARGO_MANIFEST_DIR (compile-time) so it
/// works regardless of the process working directory at runtime.
fn resolve_python() -> String {
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
fn parse_analyzer_stdout(stdout: &[u8]) -> Result<serde_json::Value, String> {
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
fn python_result_to_analysis(json: &serde_json::Value) -> models::AnalysisResult {
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
async fn analyze_one_track(
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

fn resolve_analysis_work(
    lib: &LocalLibrary,
    ids: &[i64],
) -> Result<Vec<AnalysisWorkItem>, String> {
    let mut work = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(track) = lib.get_track(id).map_err(|e| e.to_string())? {
            work.push((id, track.source_path));
        }
    }
    Ok(work)
}

fn merge_into_analysis_queue(
    coordinator: &AnalysisCoordinator,
    items: Vec<AnalysisWorkItem>,
    front: bool,
) -> u32 {
    let mut added = 0u32;
    coordinator.with_queue(|queue| {
        for item in items {
            let track_id = item.0;
            if let Some(pos) = queue.iter().position(|(id, _)| *id == track_id) {
                if front && pos > 0 {
                    let existing = queue.remove(pos).unwrap();
                    queue.push_front(existing);
                }
                continue;
            }
            if front {
                queue.push_front(item);
            } else {
                queue.push_back(item);
            }
            added += 1;
        }
    });
    if added > 0 {
        coordinator.bump_total(added);
    }
    added
}

/// Move a waiting track to the front of the analysis queue (next slot when a worker frees up).
#[tauri::command]
fn prioritize_analysis(
    track_id: i64,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<(), String> {
    coordinator.with_queue(|queue| {
        if let Some(pos) = queue.iter().position(|(id, _)| *id == track_id) {
            if pos > 0 {
                let item = queue.remove(pos).unwrap();
                queue.push_front(item);
            }
        }
    });
    Ok(())
}

/// Append or prepend tracks to the active analysis run (no-op when idle).
#[tauri::command]
fn enqueue_analysis(
    ids: Vec<i64>,
    front: bool,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<u32, String> {
    if !coordinator.is_active() {
        return Ok(0);
    }
    let lib = state.lock().map_err(|e| e.to_string())?;
    let items = resolve_analysis_work(&lib, &ids)?;
    Ok(merge_into_analysis_queue(&coordinator, items, front))
}

/// Run Python analysis for the given track ids (paths). Used by bulk and per-selection commands.
async fn run_track_analysis(
    app: AppHandle,
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
) -> Result<Vec<TrackInfo>, String> {
    if pending.is_empty() {
        debug_info(&app, "No unanalyzed tracks — nothing to do");
        let lib = shared.lock().map_err(|e| e.to_string())?;
        return build_track_infos(&lib);
    }

    if !coordinator.try_begin() {
        merge_into_analysis_queue(&coordinator, pending, false);
        let lib = shared.lock().map_err(|e| e.to_string())?;
        return build_track_infos(&lib);
    }

    let python = resolve_python();
    debug_info(
        &app,
        &format!(
            "Starting analysis: {} tracks, python={}",
            pending.len(),
            python
        ),
    );

    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from(pending)));
    let initial_total = {
        let q = queue.lock().map_err(|e| e.to_string())?;
        q.len() as u32
    };
    coordinator.register_run(queue.clone(), initial_total);
    struct RunGuard {
        coordinator: AnalysisCoordinator,
    }
    impl Drop for RunGuard {
        fn drop(&mut self) {
            self.coordinator.end_run();
        }
    }
    let _run_guard = RunGuard {
        coordinator: coordinator.clone(),
    };

    const MAX_IN_FLIGHT: usize = 3;
    let mut join_set: tokio::task::JoinSet<(i64, PathBuf, Result<(models::AnalysisResult, String), String>)> =
        tokio::task::JoinSet::new();
    let mut completed: u32 = 0;
    let mut errors: u32 = 0;

    loop {
        while join_set.len() < MAX_IN_FLIGHT {
            let next = {
                let mut q = queue.lock().map_err(|e| e.to_string())?;
                q.pop_front()
            };
            let Some((track_id, source_path)) = next else {
                break;
            };
            let python_cmd = python.clone();
            join_set.spawn(async move {
                let result = analyze_one_track(&python_cmd, track_id, &source_path).await;
                (track_id, source_path, result)
            });
        }

        if join_set.is_empty() {
            break;
        }

        let task_result = join_set
            .join_next()
            .await
            .ok_or_else(|| "analysis task ended".to_string())?;
        let (track_id, source_path, analysis_result) =
            task_result.map_err(|e| format!("Task panicked: {e}"))?;

        completed += 1;
        let total = coordinator.total().max(completed);
        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        match analysis_result {
            Ok((result, detail)) => {
                debug_ok(&app, &format!(
                    "[{completed}/{total}] {file_name} — bpm={:.1} key={} beats={} cues={} {detail}",
                    result.bpm,
                    result.key,
                    result.beat_grid.beats.len(),
                    result.cue_points.len(),
                ));
                let t_db = std::time::Instant::now();
                let lib = shared.lock().map_err(|e| e.to_string())?;
                if let Some(mut track) = lib.get_track(track_id).map_err(|e| e.to_string())? {
                    track.tempo = (result.bpm * 100.0) as u32;
                    track.key = result.key.clone();
                    lib.update_track(track_id, &track).map_err(|e| e.to_string())?;
                }
                lib.set_analysis(track_id, &result).map_err(|e| e.to_string())?;
                debug_dim(
                    &app,
                    &format!(
                        "[{completed}/{total}] {file_name} DB+ANLZ write {:.1}s",
                        t_db.elapsed().as_secs_f64()
                    ),
                );

                app.emit(
                    "analysis-progress",
                    ProgressPayload {
                        current: completed,
                        total,
                        message: format!("Analyzed {file_name} ({completed}/{total})"),
                        track_id: Some(track_id as u32),
                    },
                )
                .ok();
            }
            Err(e) => {
                errors += 1;
                debug_err(&app, &format!(
                    "[{completed}/{total}] {file_name} — FAILED: {e}",
                ));
                app.emit(
                    "analysis-progress",
                    ProgressPayload {
                        current: completed,
                        total,
                        message: format!("Failed {file_name} ({completed}/{total})"),
                        track_id: Some(track_id as u32),
                    },
                )
                .ok();
            }
        }
    }

    debug_info(&app, &format!(
        "Analysis complete: {completed} done, {errors} errors out of {}",
        coordinator.total()
    ));

    app.emit("analysis-complete", ()).ok();

    if errors > 0 && errors == coordinator.total() {
        return Err(format!(
            "Analysis failed for all {} tracks — see debug log",
            coordinator.total()
        ));
    }

    let lib = shared.lock().map_err(|e| e.to_string())?;
    build_track_infos(&lib)
}

/// Analyze all tracks that have not yet been analyzed.
#[tauri::command]
async fn analyze_tracks(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<Vec<TrackInfo>, String> {
    let shared: SharedLibrary = state.inner().clone();
    let pending: Vec<AnalysisWorkItem> = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let unanalyzed_ids = lib.get_unanalyzed_track_ids().map_err(|e| e.to_string())?;
        resolve_analysis_work(&lib, &unanalyzed_ids)?
    };
    run_track_analysis(app, shared, coordinator.inner().clone(), pending).await
}

/// Analyze or re-analyze specific tracks by library id.
#[tauri::command]
async fn analyze_track_ids(
    ids: Vec<i64>,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<Vec<TrackInfo>, String> {
    let shared: SharedLibrary = state.inner().clone();
    let pending: Vec<AnalysisWorkItem> = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        resolve_analysis_work(&lib, &ids)?
    };
    run_track_analysis(app, shared, coordinator.inner().clone(), pending).await
}

/// Incrementally sync the Pioneer USB structure to the given output directory.
///
/// Only tracks that have been analyzed are included because the writer
/// requires an `AnalysisResult` for every track. Unchanged tracks are
/// skipped (no audio re-copy), and removed tracks are cleaned up.
#[tauri::command]
fn write_usb(
    output_dir: String,
    playlists: Vec<PlaylistInput>,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<models::SyncReport, String> {
    debug_info(&app, &format!("USB sync → {output_dir}"));

    let lib = state.lock().map_err(|e| e.to_string())?;

    // Save playlists to library before writing
    sync_playlists_to_library(&lib, &playlists)?;

    let out = Path::new(&output_dir);
    let report = lib.sync_usb(out).map_err(|e| e.to_string())?;

    debug_ok(&app, &format!(
        "Sync done: +{} ~{} ↑{} −{} ({} unchanged)",
        report.tracks_added, report.tracks_updated,
        report.tracks_replaced, report.tracks_removed,
        report.tracks_unchanged,
    ));

    app.emit("write-complete", &report).ok();

    Ok(report)
}

/// Sync frontend playlists into the library.
fn sync_playlists_to_library(
    lib: &LocalLibrary,
    playlists: &[PlaylistInput],
) -> Result<(), String> {
    // Delete all existing playlists and recreate from frontend state.
    let existing = lib.get_all_playlists().map_err(|e| e.to_string())?;
    for pl in &existing {
        lib.delete_playlist(pl.id as i64)
            .map_err(|e| e.to_string())?;
    }
    for pl in playlists {
        let id = lib
            .create_playlist(&pl.name)
            .map_err(|e| e.to_string())?;
        let track_ids: Vec<i64> = pl.track_ids.iter().map(|&tid| tid as i64).collect();
        lib.set_playlist_tracks(id, &track_ids)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove tracks by ID from the library.
#[tauri::command]
fn remove_tracks(ids: Vec<u32>, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    let ids_i64: Vec<i64> = ids.into_iter().map(|id| id as i64).collect();
    lib.remove_tracks(&ids_i64).map_err(|e| e.to_string())
}

/// Set 2 test hot cues (A at 1:00, B at 1:30) on the selected tracks.
/// Replaces any existing cues on those tracks.
#[tauri::command]
fn set_test_cues(
    ids: Vec<u32>,
    state: State<'_, SharedLibrary>,
) -> Result<Vec<TrackInfo>, String> {
    let lib = state.lock().map_err(|e| e.to_string())?;

    for &track_id in &ids {
        let Some(existing) = lib
            .get_analysis(track_id as i64)
            .map_err(|e| e.to_string())?
        else {
            continue;
        };

        let new_analysis = models::AnalysisResult {
            cue_points: vec![
                models::CuePoint {
                    hot_cue_number: 0, // Memory cue
                    time_ms: 60_000,
                    loop_time_ms: None,
                },
                models::CuePoint {
                    hot_cue_number: 1, // Hot cue A
                    time_ms: 75_000,
                    loop_time_ms: None,
                },
                models::CuePoint {
                    hot_cue_number: 2, // Hot cue B
                    time_ms: 90_000,
                    loop_time_ms: None,
                },
            ],
            beat_grid: existing.beat_grid,
            waveform: existing.waveform,
            bpm: existing.bpm,
            key: existing.key,
            energy: existing.energy,
            color_waveform: None,
        };
        lib.set_analysis(track_id as i64, &new_analysis)
            .map_err(|e| e.to_string())?;
    }

    build_track_infos(&lib)
}

/// List mounted volumes so the frontend can offer a target-drive picker.
#[tauri::command]
fn get_mounted_volumes() -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let volumes = Path::new("/Volumes");
        let mut result = Vec::new();
        let entries = std::fs::read_dir(volumes).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            if let Some(name) = entry.path().to_str() {
                result.push(name.to_string());
            }
        }
        result.sort();
        Ok(result)
    }

    #[cfg(target_os = "windows")]
    {
        let mut result = Vec::new();
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            if Path::new(&drive).exists() {
                result.push(drive);
            }
        }
        Ok(result)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut result = Vec::new();
        for dir in &["/media", "/mnt"] {
            let base = Path::new(dir);
            if base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.path().to_str() {
                            result.push(name.to_string());
                        }
                    }
                }
            }
        }
        result.sort();
        Ok(result)
    }
}

/// Eject a mounted volume (macOS: `diskutil eject`, others: `umount`).
#[tauri::command]
fn eject_volume(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("diskutil")
            .args(["eject", &path])
            .output()
            .map_err(|e| format!("Failed to run diskutil: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Eject failed: {stderr}"));
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let output = std::process::Command::new("umount")
            .arg(&path)
            .output()
            .map_err(|e| format!("Failed to run umount: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Eject failed: {stderr}"));
        }
    }

    Ok(())
}

/// Wipe all Pioneer data from a USB drive (removes PIONEER/ and Contents/ directories).
#[tauri::command]
fn wipe_usb(path: String) -> Result<(), String> {
    let usb = std::path::Path::new(&path);
    if !usb.is_dir() {
        return Err("Not a valid directory".into());
    }

    let pioneer_dir = usb.join("PIONEER");
    let contents_dir = usb.join("Contents");

    if pioneer_dir.exists() {
        std::fs::remove_dir_all(&pioneer_dir)
            .map_err(|e| format!("Failed to remove PIONEER/: {e}"))?;
    }
    if contents_dir.exists() {
        std::fs::remove_dir_all(&contents_dir)
            .map_err(|e| format!("Failed to remove Contents/: {e}"))?;
    }

    Ok(())
}

/// Return the application version string.
#[tauri::command]
fn app_version() -> String {
    pioneer_usb_writer::VERSION.to_string()
}

/// Run Python analysis CLI on a single track and return the result as JSON.
/// Uses the venv Python at `analysis/.venv/bin/python`.
#[tauri::command]
async fn analyze_track_python(path: String) -> Result<serde_json::Value, String> {
    let python = resolve_python();

    let output = tokio::task::spawn_blocking(move || {
        let child = std::process::Command::new(&python)
            .args(["-m", "fourfour_analysis", "analyze", &path, "--backend", "deeprhythm_essentia", "--json"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn analyzer: {e}"))?;

        let timeout = std::time::Duration::from_secs(120);
        let wait_result = std::thread::scope(|s| {
            let handle = s.spawn(move || child.wait_with_output());
            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    return Some(handle.join().expect("wait thread panicked"));
                }
                if start.elapsed() > timeout {
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        match wait_result {
            Some(Ok(output)) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("Python analyzer failed: {stderr}"));
                }
                let results: Vec<serde_json::Value> =
                    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                results.into_iter().next().ok_or_else(|| "No results".to_string())
            }
            Some(Err(e)) => Err(format!("Analyzer wait failed: {e}")),
            None => Err(format!("Analyzer timed out after {}s", timeout.as_secs())),
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    output
}

/// Get stored analysis data for a track from the local library DB.
#[tauri::command]
async fn get_analysis_data(
    track_id: i64,
    state: State<'_, SharedLibrary>,
) -> Result<serde_json::Value, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        analysis_payload::build_analysis_json(&lib, track_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Get raw artwork bytes for a track from the local library DB.
/// Sent as a binary IPC response (an `ArrayBuffer`), not a JSON number array.
#[tauri::command]
async fn get_track_artwork(
    track_id: i64,
    state: State<'_, SharedLibrary>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.get_artwork(track_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes.unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// USB state reading
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct UsbTrackInfo {
    id: u32,
    title: String,
    artist: String,
    album: String,
    genre: String,
    key: String,
    bpm: f64,
    duration: f64,
    usb_path: String,
}

#[derive(Serialize)]
struct UsbPlaylistInfo {
    id: u32,
    name: String,
    track_count: usize,
}

#[derive(Serialize)]
struct UsbStateResponse {
    tracks: Vec<UsbTrackInfo>,
    playlists: Vec<UsbPlaylistInfo>,
}

/// Read the existing Pioneer USB library state from the OneLibrary database.
/// Returns `None` if no `exportLibrary.db` is found at the expected path.
#[tauri::command]
fn read_usb_state(path: String) -> Result<Option<UsbStateResponse>, String> {
    let state = pioneer_usb_writer::reader::read_usb_state(Path::new(&path))
        .map_err(|e| e.to_string())?;

    let Some(existing) = state else {
        return Ok(None);
    };

    let tracks = existing
        .tracks
        .into_iter()
        .map(|track| UsbTrackInfo {
            id: track.id,
            title: track.title,
            artist: track.artist,
            album: track.album,
            genre: track.genre,
            key: track.key,
            bpm: track.tempo as f64 / 100.0,
            duration: track.duration_secs,
            usb_path: track.usb_path,
        })
        .collect();

    let playlists = existing
        .playlists
        .into_iter()
        .map(|playlist| UsbPlaylistInfo {
            id: playlist.id,
            name: playlist.name,
            track_count: playlist.track_ids.len(),
        })
        .collect();

    Ok(Some(UsbStateResponse { tracks, playlists }))
}

// ---------------------------------------------------------------------------
// Persistence (backed by LocalLibrary — no more JSON)
// ---------------------------------------------------------------------------

/// Save playlists to the library. Tracks are already persisted automatically.
#[tauri::command]
fn save_state(
    playlists: Vec<PlaylistInput>,
    _app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<(), String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    sync_playlists_to_library(&lib, &playlists)
}

/// Load the library state. Returns all tracks and playlists.
#[tauri::command]
async fn load_state(state: State<'_, SharedLibrary>) -> Result<LoadedState, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;

        let track_infos = build_track_infos(&lib)?;

        let playlists = lib
            .get_all_playlists()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|pl| PlaylistInput {
                id: pl.id,
                name: pl.name,
                track_ids: pl.track_ids,
            })
            .collect();

        Ok(LoadedState {
            tracks: track_infos,
            playlists,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Library path management
// ---------------------------------------------------------------------------

/// Config file that remembers the user's chosen library path.
fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

fn default_library_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("library.db"))
}

/// Read the stored library path from config, falling back to the default.
fn read_library_path(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = config_path(app)?;
    if cfg.exists() {
        let json = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(p) = val.get("library_path").and_then(|v| v.as_str()) {
                return Ok(PathBuf::from(p));
            }
        }
    }
    default_library_path(app)
}

/// Save the library path to config.
fn write_library_path(app: &AppHandle, path: &Path) -> Result<(), String> {
    let cfg = config_path(app)?;
    let json = serde_json::json!({ "library_path": path.to_string_lossy() });
    std::fs::write(&cfg, json.to_string()).map_err(|e| e.to_string())
}

/// Return the path of the currently open library database.
#[tauri::command]
fn get_library_path(app: AppHandle) -> Result<String, String> {
    let path = read_library_path(&app)?;
    Ok(path.to_string_lossy().to_string())
}

/// Open or create a library at the given directory path.
/// Creates `library.db` inside the chosen directory and swaps the active library.
#[tauri::command]
fn change_library_path(
    folder_path: String,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<String, String> {
    let db_path = PathBuf::from(&folder_path).join("library.db");

    let new_library = LocalLibrary::open(&db_path).map_err(|e| e.to_string())?;

    // Swap the active library
    let mut lib = state.lock().map_err(|e| e.to_string())?;
    *lib = new_library;

    // Remember the choice
    write_library_path(&app, &db_path)?;

    Ok(db_path.to_string_lossy().to_string())
}

/// Delete the on-disk library database (and ANLZ cache), then open a fresh empty library
/// at the configured path. Playlists and tracks are wiped; `config.json` library_path is kept.
#[tauri::command]
fn reset_library(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<String, String> {
    let db_path = read_library_path(&app)?;

    // Release file handles before deleting SQLite files.
    {
        let mut lib = state.lock().map_err(|e| e.to_string())?;
        *lib = LocalLibrary::open_in_memory().map_err(|e| e.to_string())?;
    }

    remove_library_files(&db_path)?;

    let fresh = LocalLibrary::open(&db_path).map_err(|e| e.to_string())?;
    {
        let mut lib = state.lock().map_err(|e| e.to_string())?;
        *lib = fresh;
    }

    Ok(db_path.to_string_lossy().to_string())
}

/// Remove SQLite library files and the sibling `anlz/` analysis cache directory.
fn remove_library_files(db_path: &Path) -> Result<(), String> {
    let base = db_path.to_string_lossy();
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{base}{suffix}"));
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        }
    }

    if let Some(parent) = db_path.parent() {
        let anlz = parent.join("anlz");
        if anlz.exists() {
            std::fs::remove_dir_all(&anlz)
                .map_err(|e| format!("Failed to remove {}: {e}", anlz.display()))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("focus_main_window: no webview labeled \"main\"");
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn main() {
    tauri::Builder::default() // v0.1
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            scan_files,
            analyze_tracks,
            analyze_track_ids,
            enqueue_analysis,
            prioritize_analysis,
            write_usb,
            remove_tracks,
            set_test_cues,
            get_mounted_volumes,
            eject_volume,
            wipe_usb,
            save_state,
            load_state,
            app_version,
            read_usb_state,
            get_library_path,
            change_library_path,
            reset_library,
            analyze_track_python,
            get_analysis_data,
            get_track_artwork,
            playback_play,
            playback_pause,
            playback_resume,
            playback_seek,
            playback_set_position,
            playback_stop,
            playback_configure,
            playback_get_config,
            playback_list_output_devices,
            playback_get_output_device,
            playback_set_output_device,
        ])
        .setup(|app| {
            // Open (or create) the local library database at the stored/default path
            let db_path = read_library_path(&app.handle())
                .expect("Failed to determine library database path");
            let library = LocalLibrary::open(&db_path)
                .expect("Failed to open library database");
            app.manage(Arc::new(Mutex::new(library)));
            app.manage(AnalysisCoordinator::default());

            match PlaybackEngine::new() {
                Ok(playback) => {
                    app.manage(Arc::new(playback));
                }
                Err(e) => eprintln!("audio playback unavailable: {e}"),
            }

            focus_main_window(&app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Ready => {
                    focus_main_window(&app_handle);
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => focus_main_window(&app_handle),
                _ => {}
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{parse_analyzer_stdout, python_result_to_analysis};

    #[test]
    fn parse_backend_keyed_analyzer_json() {
        let stdout = br#"{"deeprhythm_essentia":{"bpm":128.0,"key":"8A","waveform_preview":[63]}}"#;
        let json = parse_analyzer_stdout(stdout).expect("parse");
        let analysis = python_result_to_analysis(&json);
        assert!((analysis.bpm - 128.0).abs() < f64::EPSILON);
        assert_eq!(analysis.key, "8A");
    }

    #[test]
    fn parse_legacy_array_analyzer_json() {
        let stdout = br#"[{"bpm":120.0,"key":"1A","waveform_preview":[1,2,3]}]"#;
        let json = parse_analyzer_stdout(stdout).expect("parse");
        let analysis = python_result_to_analysis(&json);
        assert!((analysis.bpm - 120.0).abs() < f64::EPSILON);
    }
}
