#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dto;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use pioneer_usb_writer::models;
use pioneer_usb_writer::{analyzer, scanner, writer};

use dto::{LoadedState, PlaylistInput, ProgressPayload, TrackInfo};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

struct AppState {
    tracks: Vec<models::Track>,
    /// Parallel to `tracks` — `None` means "not yet analyzed".
    analyses: Vec<Option<models::AnalysisResult>>,
    next_id: u32,
}

type SharedState = Arc<Mutex<AppState>>;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Scan a directory for audio files and add them to the state.
#[tauri::command]
fn scan_directory(
    path: String,
    state: State<'_, SharedState>,
) -> Result<Vec<TrackInfo>, String> {
    let dir = Path::new(&path);
    let mut scanned = scanner::scan_directory(dir).map_err(|e| e.to_string())?;

    let mut st = state.lock().map_err(|e| e.to_string())?;

    // Assign IDs from the global counter so they don't collide with
    // previously loaded tracks.
    for track in &mut scanned {
        track.id = st.next_id;
        st.next_id += 1;
    }

    let infos: Vec<TrackInfo> = scanned.iter().map(TrackInfo::from).collect();

    // Push into shared state with empty analyses.
    for track in scanned {
        st.tracks.push(track);
        st.analyses.push(None);
    }

    Ok(infos)
}

/// Scan specific files and/or folders and add them to the state.
/// Directories are recursed into via scan_directory; files are scanned directly.
#[tauri::command]
fn scan_files(
    paths: Vec<String>,
    state: State<'_, SharedState>,
) -> Result<Vec<TrackInfo>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dir_tracks: Vec<models::Track> = Vec::new();

    for p in &paths {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            let mut found = scanner::scan_directory(&pb).map_err(|e| e.to_string())?;
            // IDs will be reassigned below; zero them out to avoid conflicts
            for t in &mut found { t.id = 0; }
            dir_tracks.extend(found);
        } else {
            files.push(pb);
        }
    }

    let mut scanned = scanner::scan_files(&files).map_err(|e| e.to_string())?;
    scanned.extend(dir_tracks);

    let mut st = state.lock().map_err(|e| e.to_string())?;

    for track in &mut scanned {
        track.id = st.next_id;
        st.next_id += 1;
    }

    let infos: Vec<TrackInfo> = scanned.iter().map(TrackInfo::from).collect();

    for track in scanned {
        st.tracks.push(track);
        st.analyses.push(None);
    }

    Ok(infos)
}

/// Analyze all tracks that have not yet been analyzed.
///
/// Emits `analysis-progress` events so the frontend can show a progress bar.
/// Returns the full (updated) track list.
///
/// Runs each track analysis on a blocking thread so the Tauri async runtime
/// stays responsive (progress events keep flowing, macOS won't kill the app).
#[tauri::command]
async fn analyze_tracks(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<TrackInfo>, String> {
    let shared: SharedState = state.inner().clone();

    // 1. Collect the work we need to do while holding the lock briefly.
    let pending: Vec<(usize, PathBuf)> = {
        let st = shared.lock().map_err(|e| e.to_string())?;
        st.analyses
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_none())
            .map(|(i, _)| (i, st.tracks[i].source_path.clone()))
            .collect()
    };

    let total = pending.len() as u32;

    // 2. Run analysis on a blocking thread, one track at a time.
    for (seq, (idx, source_path)) in pending.iter().enumerate() {
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
        let analysis_result = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analyzer::analyze_track(&path)
            }))
        })
        .await
        .map_err(|e| format!("Analysis task failed: {e}"))?;

        match analysis_result {
            Ok(Ok(result)) => {
                // 3. Lock briefly to store result and update track metadata.
                let mut st = shared.lock().map_err(|e| e.to_string())?;
                st.tracks[*idx].tempo = (result.bpm * 100.0) as u32;
                st.tracks[*idx].key = result.key.clone();
                st.analyses[*idx] = Some(result);
            }
            Ok(Err(e)) => {
                eprintln!(
                    "Warning: analysis failed for {}: {}",
                    source_path.display(),
                    e
                );
            }
            Err(_panic) => {
                eprintln!(
                    "Warning: analysis panicked for {}",
                    source_path.display(),
                );
            }
        }
    }

    // 4. Return the full updated track list (with cue info).
    let st = shared.lock().map_err(|e| e.to_string())?;
    let infos: Vec<TrackInfo> = st.tracks.iter().zip(st.analyses.iter())
        .map(|(t, a)| TrackInfo::from_track_and_analysis(t, a.as_ref()))
        .collect();
    Ok(infos)
}

/// Write the Pioneer USB structure to the given output directory.
///
/// Only tracks that have been analyzed are included because the writer
/// requires an `AnalysisResult` for every track.
#[tauri::command]
fn write_usb(
    output_dir: String,
    playlists: Vec<PlaylistInput>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let st = state.lock().map_err(|e| e.to_string())?;

    // Collect only analyzed tracks (writer needs parallel slices).
    let mut analyzed_tracks: Vec<models::Track> = Vec::new();
    let mut analyzed_results: Vec<models::AnalysisResult> = Vec::new();

    for (track, analysis) in st.tracks.iter().zip(st.analyses.iter()) {
        if let Some(a) = analysis {
            analyzed_tracks.push(track.clone());
            analyzed_results.push(a.clone());
        }
    }

    // Convert PlaylistInput -> models::Playlist
    let model_playlists: Vec<models::Playlist> = playlists
        .into_iter()
        .map(|p| models::Playlist {
            id: p.id,
            name: p.name,
            track_ids: p.track_ids,
        })
        .collect();

    // Drop the lock before the potentially long write operation.
    drop(st);

    let out = Path::new(&output_dir);
    writer::filesystem::write_usb(out, &analyzed_tracks, &analyzed_results, &model_playlists)
        .map_err(|e| e.to_string())?;

    app.emit(
        "write-complete",
        ProgressPayload {
            current: 1,
            total: 1,
            message: "USB write complete".to_string(),
        },
    )
    .ok();

    Ok(())
}

/// Remove tracks by ID from the shared state and all playlists.
#[tauri::command]
fn remove_tracks(ids: Vec<u32>, state: State<'_, SharedState>) -> Result<(), String> {
    let id_set: std::collections::HashSet<u32> = ids.into_iter().collect();
    let mut st = state.lock().map_err(|e| e.to_string())?;
    let mut i = 0;
    while i < st.tracks.len() {
        if id_set.contains(&st.tracks[i].id) {
            st.tracks.remove(i);
            st.analyses.remove(i);
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Set 2 test hot cues (A at 1:00, B at 1:30) on the selected tracks.
/// Replaces any existing cues on those tracks.
#[tauri::command]
fn set_test_cues(
    ids: Vec<u32>,
    state: State<'_, SharedState>,
) -> Result<Vec<TrackInfo>, String> {
    let id_set: std::collections::HashSet<u32> = ids.into_iter().collect();
    let mut st = state.lock().map_err(|e| e.to_string())?;

    for i in 0..st.tracks.len() {
        if !id_set.contains(&st.tracks[i].id) {
            continue;
        }
        let Some(ref existing) = st.analyses[i] else {
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
            beat_grid: existing.beat_grid.clone(),
            waveform: existing.waveform.clone(),
            bpm: existing.bpm,
            key: existing.key.clone(),
        };
        st.analyses[i] = Some(new_analysis);
    }

    let infos: Vec<TrackInfo> = st.tracks.iter().zip(st.analyses.iter())
        .map(|(t, a)| TrackInfo::from_track_and_analysis(t, a.as_ref()))
        .collect();
    Ok(infos)
}

/// Open a native folder-picker dialog and return the selected path.
///
/// Uses the blocking variant of the dialog plugin, which is safe to call from
/// a Tauri command thread (commands do not run on the main thread).
#[tauri::command]
fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    let result = app.dialog().file().blocking_pick_folder();

    match result {
        Some(file_path) => {
            let path_buf: PathBuf = file_path
                .into_path()
                .map_err(|e| format!("Invalid path: {e}"))?;
            Ok(Some(path_buf.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
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
        // Fallback for Linux — list /media and /mnt
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

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Everything we write to disk.
#[derive(Serialize, Deserialize)]
struct PersistentState {
    tracks: Vec<models::Track>,
    analyses: Vec<Option<models::AnalysisResult>>,
    playlists: Vec<PlaylistInput>,
    next_id: u32,
}

fn state_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("state.json"))
}

/// Save the current tracks, analyses, and playlists to disk.
#[tauri::command]
fn save_state(
    playlists: Vec<PlaylistInput>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let st = state.lock().map_err(|e| e.to_string())?;
    let persistent = PersistentState {
        tracks: st.tracks.clone(),
        analyses: st.analyses.clone(),
        playlists,
        next_id: st.next_id,
    };
    drop(st);

    let path = state_file_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(&persistent).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load previously saved state from disk. Returns tracks + playlists for the
/// frontend and repopulates the backend shared state.
#[tauri::command]
fn load_state(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<LoadedState, String> {
    let path = state_file_path(&app)?;
    if !path.exists() {
        return Ok(LoadedState {
            tracks: Vec::new(),
            playlists: Vec::new(),
        });
    }

    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let persistent: PersistentState =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse saved state: {e}"))?;

    let infos: Vec<TrackInfo> = persistent.tracks.iter().map(TrackInfo::from).collect();

    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.tracks = persistent.tracks;
    st.analyses = persistent.analyses;
    st.next_id = persistent.next_id;

    Ok(LoadedState {
        tracks: infos,
        playlists: persistent.playlists,
    })
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
        .manage(Arc::new(Mutex::new(AppState {
            tracks: Vec::new(),
            analyses: Vec::new(),
            next_id: 1,
        })))
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            scan_files,
            analyze_tracks,
            write_usb,
            remove_tracks,
            set_test_cues,
            pick_directory,
            get_mounted_volumes,
            eject_volume,
            wipe_usb,
            save_state,
            load_state,
            app_version,
        ])
        .setup(|app| {
            // Ensure the window is positioned on the primary monitor and focused.
            // This works around tiling window managers (e.g. AeroSpace) that may
            // not recognise un-bundled binaries and leave the window off-screen.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.center();
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Activate the app so macOS sends keyboard input to our window
            // (always_on_top alone makes the window float but keystrokes go to
            // whichever app was frontmost before)
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
                let current_app = NSRunningApplication::currentApplication();
                let _ = current_app.activateWithOptions(
                    NSApplicationActivationOptions::ActivateIgnoringOtherApps
                );
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
