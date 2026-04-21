#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dto;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

mod analyzer;

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;
use pioneer_usb_writer::scanner;

use dto::{LoadedState, PlaylistInput, ProgressPayload, TrackInfo};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

type SharedLibrary = Arc<Mutex<LocalLibrary>>;

/// Build TrackInfo list from the library's tracks + flags.
fn build_track_infos(lib: &LocalLibrary) -> Result<Vec<TrackInfo>, String> {
    let rows = lib.get_all_tracks_with_flags().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(track, has_artwork, _has_analysis, has_cues)| TrackInfo {
            id: track.id,
            source_path: track.source_path.to_string_lossy().to_string(),
            title: track.title,
            artist: track.artist,
            album: track.album,
            genre: track.genre,
            tempo: track.tempo,
            key: track.key,
            duration_secs: track.duration_secs,
            bitrate: track.bitrate,
            file_size: track.file_size,
            has_artwork,
            has_cues,
        })
        .collect())
}

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

/// Convert a Python fourfour_analysis JSON result to `models::AnalysisResult`.
fn python_result_to_analysis(json: &serde_json::Value) -> models::AnalysisResult {
    let bpm = json.get("bpm").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let key = json
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Build waveform preview from the 400-int array.
    let mut waveform_data = [0u8; 400];
    if let Some(arr) = json.get("waveform_preview").and_then(|v| v.as_array()) {
        for (i, val) in arr.iter().enumerate().take(400) {
            waveform_data[i] = val.as_u64().unwrap_or(0) as u8;
        }
    }

    models::AnalysisResult {
        bpm,
        key,
        beat_grid: models::BeatGrid { beats: Vec::new() },
        waveform: models::WaveformPreview { data: waveform_data },
        cue_points: Vec::new(),
        color_waveform: None,
    }
}

/// Analyze all tracks that have not yet been analyzed.
///
/// Emits `analysis-progress` events so the frontend can show a progress bar.
/// Returns the full (updated) track list.
#[tauri::command]
async fn analyze_tracks(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<Vec<TrackInfo>, String> {
    let shared: SharedLibrary = state.inner().clone();

    // 1. Collect the work we need to do while holding the lock briefly.
    let pending: Vec<(i64, PathBuf)> = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let unanalyzed_ids = lib.get_unanalyzed_track_ids().map_err(|e| e.to_string())?;
        let mut work = Vec::with_capacity(unanalyzed_ids.len());
        for id in unanalyzed_ids {
            if let Some(track) = lib.get_track(id).map_err(|e| e.to_string())? {
                work.push((id, track.source_path));
            }
        }
        work
    };

    let total = pending.len() as u32;

    // Resolve the venv Python path once.
    let python = resolve_python();

    // 2. Run analysis one track at a time (Python CLI or Rust fallback).
    for (seq, (track_id, source_path)) in pending.iter().enumerate() {
        let current = seq as u32 + 1;

        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        app.emit(
            "analysis-progress",
            ProgressPayload {
                current,
                total,
                message: format!("Analyzing {file_name} ({current}/{total})"),
            },
        )
        .ok();

        let path = source_path.clone();
        let python_cmd = python.clone();

        // Try Python analyzer first, fall back to Rust.
        let analysis_result = tokio::task::spawn_blocking(move || {
            // Attempt Python CLI
            let path_str = path.to_string_lossy().to_string();
            let output = std::process::Command::new(&python_cmd)
                .args(["-m", "fourfour_analysis", "analyze", &path_str, "--json"])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    if let Ok(results) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) {
                        if let Some(result) = results.into_iter().next() {
                            return Ok(python_result_to_analysis(&result));
                        }
                    }
                }
            }

            // Fallback to Rust analyzer
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analyzer::analyze_track(&path)
            }))
            .map_err(|_| anyhow::anyhow!("analysis panicked"))?
        })
        .await
        .map_err(|e| format!("Analysis task failed: {e}"))?;

        match analysis_result {
            Ok(result) => {
                // 3. Lock briefly to store result and update track metadata.
                let lib = shared.lock().map_err(|e| e.to_string())?;
                if let Some(mut track) = lib.get_track(*track_id).map_err(|e| e.to_string())? {
                    track.tempo = (result.bpm * 100.0) as u32;
                    track.key = result.key.clone();
                    lib.update_track(*track_id, &track)
                        .map_err(|e| e.to_string())?;
                }
                lib.set_analysis(*track_id, &result)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => {
                eprintln!(
                    "Warning: analysis failed for {}: {}",
                    source_path.display(),
                    e
                );
            }
        }
    }

    // 4. Return the full updated track list.
    let lib = shared.lock().map_err(|e| e.to_string())?;
    build_track_infos(&lib)
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
    let lib = state.lock().map_err(|e| e.to_string())?;

    // Save playlists to library before writing
    sync_playlists_to_library(&lib, &playlists)?;

    let out = Path::new(&output_dir);
    let report = lib.sync_usb(out).map_err(|e| e.to_string())?;

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
        std::process::Command::new(&python)
            .args(["-m", "fourfour_analysis", "analyze", &path, "--json"])
            .output()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
    .map_err(|e| format!("Failed to run Python analyzer: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python analyzer failed: {stderr}"));
    }

    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;

    results.into_iter().next().ok_or_else(|| "No results".to_string())
}

/// Get stored analysis data for a track from the local library DB.
#[tauri::command]
fn get_analysis_data(
    track_id: i64,
    state: State<'_, SharedLibrary>,
) -> Result<serde_json::Value, String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    let analysis = lib.get_analysis(track_id).map_err(|e| e.to_string())?;

    match analysis {
        Some(a) => Ok(serde_json::json!({
            "waveform_preview": a.waveform.data.to_vec(),
            "waveform_color": serde_json::Value::Array(vec![]),
            "waveform_peaks": serde_json::Value::Array(vec![]),
            "bpm": a.bpm,
            "key": a.key,
        })),
        None => Err("No analysis data for this track".to_string()),
    }
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
fn load_state(
    _app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<LoadedState, String> {
    let lib = state.lock().map_err(|e| e.to_string())?;

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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    // Set activation policy before Tauri initializes so macOS registers us
    // as a proper GUI app with a Dock icon from the start.
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        let mtm = objc2::MainThreadMarker::new().expect("must be on main thread");
        let ns_app = NSApplication::sharedApplication(mtm);
        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    }

    tauri::Builder::default() // v0.1
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            scan_files,
            analyze_tracks,
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
            analyze_track_python,
            get_analysis_data,
        ])
        .setup(|app| {
            // Open (or create) the local library database at the stored/default path
            let db_path = read_library_path(&app.handle())
                .expect("Failed to determine library database path");
            let library = LocalLibrary::open(&db_path)
                .expect("Failed to open library database");
            app.manage(Arc::new(Mutex::new(library)));

            // Ensure the window is positioned on the primary monitor and focused.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.center();
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Activate the app so macOS sends keyboard input to our window
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
                let current_app = NSRunningApplication::currentApplication();
                let _ = current_app.activateWithOptions(
                    NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                );
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
