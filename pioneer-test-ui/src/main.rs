#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis_backend;
mod analysis_order;
mod analysis_payload;
mod analysis_priority;
mod analysis_python;
mod analysis_run;
mod backup_commands;
mod band_analysis;
mod dto;
mod file_health;
mod import_plan;
mod perf_probe;
mod playback;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../fourfour-analysis-rs/tests/messy_fixtures/mod.rs"]
mod messy_fixtures;
#[cfg(all(test, unix))]
mod robustness_tests;
mod self_check;
mod tempo;
mod track_fields;
mod track_infos;
mod stick_commands;
mod volumes;
mod waveform_detail;

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use pioneer_library::rekordbox;
use pioneer_library::{BeatPosition, LocalLibrary};
use pioneer_usb_writer::models;

use analysis_backend::{AnalysisBackend, BackendEnv};
use analysis_python::resolve_python;
use analysis_run::{
    merge_into_analysis_queue, rerun_work, resolve_analysis_work, AnalysisCoordinator, AnalysisSink, AnalysisWorkItem, LogLevel,
    RunOutcome, SharedLibrary,
};
use track_fields::{apply_edits, TrackFieldEdits, TrackFields};
use track_infos::build_track_infos;
use import_plan::{ImportPlan, ImportPlans};
use dto::{LoadedState, PlaylistInput, ProgressPayload, TrackInfo};
use playback::{
    playback_configure, playback_get_config, playback_pause, playback_play, playback_resume,
    playback_get_output_device, playback_list_output_devices, playback_seek,
    playback_set_key_lock, playback_set_key_shift, playback_set_output_device,
    playback_set_position, playback_set_tempo, playback_stop, PlaybackEngine,
};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Debug logging — emits events to the frontend debug panel
// ---------------------------------------------------------------------------

fn debug_log(app: &AppHandle, level: &str, message: &str) {
    app.emit("debug-log", serde_json::json!({ "level": level, "message": message })).ok();
}

fn debug_info(app: &AppHandle, msg: &str)  { debug_log(app, "info", msg); }
fn debug_ok(app: &AppHandle, msg: &str)    { debug_log(app, "ok", msg); }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Adds the tracks not yet in the library; their covers follow from `spawn_cover_fill`, so the
/// rows come back without artwork and `import-covers` names the ones that got a cover.
fn add_scanned_tracks(lib: &LocalLibrary, scanned: Vec<models::Track>) -> Result<Vec<TrackInfo>, String> {
    let mut seen = lib.track_paths().map_err(|e| e.to_string())?;
    let fresh: Vec<models::Track> = scanned.into_iter().filter(|t| seen.insert(t.source_path.clone())).collect();
    let ids = lib.add_tracks_covers_later(&fresh).map_err(|e| e.to_string())?;
    Ok(fresh
        .iter()
        .zip(ids)
        .map(|(track, id)| TrackInfo {
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
            has_artwork: false,
            has_cues: false,
            has_analysis: false,
            analysis_pending: false,
            waveform_preview: None,
            waveform_bands: None,
            rating: None,
            color: None,
        })
        .collect())
}

/// Set while a thread stores the covers of imported tracks; one at a time.
#[derive(Default, Clone)]
struct CoverFill(Arc<std::sync::atomic::AtomicBool>);

/// How often `import-covers` may reach the list: each one re-sorts and re-filters it.
const COVER_EVENT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Stores the covers queued by imports on a thread of its own and emits `import-covers` with
/// the ids that got one, at most every [`COVER_EVENT_INTERVAL`]. A no-op while a fill is
/// already running: that one keeps going until the queue is empty.
fn spawn_cover_fill(app: AppHandle, library: SharedLibrary, fill: CoverFill) {
    use std::sync::atomic::Ordering;
    if fill.0.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || loop {
        let mut unsent: Vec<u32> = Vec::new();
        let mut last_emit = std::time::Instant::now();
        let result = import_plan::store_pending_covers(&library, None, &mut |ids| {
            unsent.extend(ids);
            if last_emit.elapsed() >= COVER_EVENT_INTERVAL {
                app.emit("import-covers", std::mem::take(&mut unsent)).ok();
                last_emit = std::time::Instant::now();
            }
        });
        if !unsent.is_empty() {
            app.emit("import-covers", unsent).ok();
        }
        fill.0.store(false, Ordering::Release);
        if let Err(e) = result {
            debug_log(&app, "error", &format!("Storing imported covers failed: {e}"));
            return;
        }
        let more = library.lock().ok().and_then(|lib| lib.pending_artwork(0, 1).ok()).is_some_and(|p| !p.is_empty());
        if !more || fill.0.swap(true, Ordering::AcqRel) {
            return;
        }
    });
}

/// Removes staging folders a crashed run left in the ANLZ folders; on a thread of its own because it
/// lists every track folder.
fn spawn_stale_staging_cleanup(db_path: PathBuf) {
    std::thread::spawn(move || {
        pioneer_library::remove_stale_staging(&db_path);
    });
}

/// Removes `.part` files a killed process left in the waveform-detail cache; on a thread of its own because
/// the cache can hold many files.
fn spawn_stale_part_sweep(cache_dir: PathBuf) {
    std::thread::spawn(move || {
        waveform_detail::sweep_stale_parts(&cache_dir, waveform_detail::STALE_PART_AGE);
    });
}

#[derive(Serialize, Clone)]
struct ImportProgress {
    done: usize,
    total: usize,
}

/// Scan files and folders and hold the result until `commit_import`, listing likely
/// duplicates of library tracks so the user can skip them.
#[tauri::command]
async fn plan_import(
    paths: Vec<String>,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    plans: State<'_, Arc<ImportPlans>>,
) -> Result<ImportPlan, String> {
    let shared = state.inner().clone();
    let plans = plans.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let known = shared.lock().map_err(|e| e.to_string())?.track_paths().map_err(|e| e.to_string())?;
        let survey = import_plan::survey_files(&paths);
        let total = survey.audio_count();
        let mut done = 0;
        let mut last_emit = std::time::Instant::now();
        app.emit("import-progress", ImportProgress { done, total }).ok();
        let scanned = import_plan::scan_paths(&survey, &known, &mut || {
            done += 1;
            if done == total || last_emit.elapsed() >= std::time::Duration::from_millis(80) {
                last_emit = std::time::Instant::now();
                app.emit("import-progress", ImportProgress { done, total }).ok();
            }
        });
        let lib = shared.lock().map_err(|e| e.to_string())?;
        import_plan::plan_for_library(&plans, &lib, scanned, &survey)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct ImportCommit {
    added: Vec<TrackInfo>,
    skipped_ids: Vec<u32>,
}

/// Add the planned tracks; with `skip_duplicates` the likely duplicates stay out.
#[tauri::command]
async fn commit_import(
    token: u64,
    skip_duplicates: bool,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    plans: State<'_, Arc<ImportPlans>>,
    covers: State<'_, CoverFill>,
) -> Result<ImportCommit, String> {
    let (tracks, skipped_ids) = plans.take(token)?.resolve(skip_duplicates);
    let shared = state.inner().clone();
    let covers = covers.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let added = add_scanned_tracks(&*shared.lock().map_err(|e| e.to_string())?, tracks)?;
        spawn_cover_fill(app, shared, covers);
        Ok(ImportCommit { added, skipped_ids })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Analyse a track as fast as possible: it leaves the queue for the now lane (all threads,
/// background analysis paused) and its waveform arrives first as `analysis-partial`.
#[tauri::command]
async fn prioritize_analysis(
    track_id: i64,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<(), String> {
    let shared: SharedLibrary = state.inner().clone();
    let item = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        resolve_analysis_work(&lib, &[track_id])?.into_iter().next()
    };
    let Some(item) = item else {
        return Ok(());
    };
    let sink = Arc::new(TauriAnalysisSink(app.clone()));
    let env = analysis_backend_env(&app);
    analysis_run::prioritize_track(shared, coordinator.inner().clone(), item, sink, env).await
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

#[derive(serde::Serialize, Clone)]
struct PartialPayload {
    track_id: u32,
}

struct TauriAnalysisSink(AppHandle);

impl AnalysisSink for TauriAnalysisSink {
    fn log(&self, level: LogLevel, message: String) {
        debug_log(&self.0, level.as_str(), &message);
    }

    fn progress(&self, current: u32, total: u32, message: String, track_id: i64) {
        self.0
            .emit(
                "analysis-progress",
                ProgressPayload { current, total, message, track_id: Some(track_id as u32) },
            )
            .ok();
    }

    fn partial(&self, track_id: i64) {
        self.0.emit("analysis-partial", PartialPayload { track_id: track_id as u32 }).ok();
    }

    fn complete(&self) {
        self.0.emit("analysis-complete", ()).ok();
    }
}

fn analysis_backend_env(app: &AppHandle) -> BackendEnv {
    BackendEnv::from_process(config_path(app).ok(), app.path().app_data_dir().ok(), app.path().resource_dir().ok())
}

/// Run analysis for the given tracks with the configured backend (rust when model
/// weights are present, python otherwise). Used by bulk and per-selection commands.
async fn run_track_analysis(
    app: AppHandle,
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
) -> Result<Vec<TrackInfo>, String> {
    let sink = Arc::new(TauriAnalysisSink(app.clone()));
    let env = analysis_backend_env(&app);
    let outcome = analysis_run::run_track_analysis(shared.clone(), coordinator, pending, sink, env).await?;
    if let RunOutcome::Finished { errors, total } = outcome {
        if errors > 0 && errors == total {
            return Err(format!("Analysis failed for all {total} tracks — see debug log"));
        }
    }
    let lib = shared.lock().map_err(|e| e.to_string())?;
    build_track_infos(&lib)
}

/// Stop the active analysis run: drop queued tracks, abort in-flight rust analyses.
#[tauri::command]
fn cancel_analysis(coordinator: State<'_, AnalysisCoordinator>) {
    coordinator.cancel();
}

#[tauri::command]
async fn analysis_ui_busy(coordinator: State<'_, AnalysisCoordinator>) -> Result<(), String> {
    if std::env::var_os("FF_ANALYSIS_UI_PAUSE").is_some_and(|v| v == "off") {
        return Ok(());
    }
    coordinator.ui_busy();
    Ok(())
}

/// Stored analysis backend setting: "rust", "python", or null for the default.
#[tauri::command]
fn get_analysis_backend(app: AppHandle) -> Option<String> {
    let path = config_path(&app).ok()?;
    analysis_backend::read_setting(&path).map(|b| b.name().to_string())
}

#[tauri::command]
fn set_analysis_backend(backend: Option<String>, app: AppHandle) -> Result<(), String> {
    let parsed = match backend.as_deref() {
        None => None,
        Some(text) => Some(AnalysisBackend::parse(text).ok_or_else(|| format!("unknown analysis backend: {text}"))?),
    };
    analysis_backend::write_setting(&config_path(&app)?, parsed)
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

/// Queue tracks left with missing stages (and, with the setting on, never-analysed ones) at launch.
/// Returns how many tracks were queued; the run continues in the background.
#[tauri::command]
fn resume_analysis(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<usize, String> {
    let shared: SharedLibrary = state.inner().clone();
    let include_new = config_path(&app).map_or(true, |path| analysis_backend::read_auto_analyze_new(&path));
    let app_data_dir = app.path().app_data_dir().ok();
    let pending = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        analysis_run::resume_work_in_saved_order(&lib, include_new, app_data_dir.as_deref())?
    };
    let queued = pending.len();
    if queued > 0 {
        let coordinator = coordinator.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = run_track_analysis(app.clone(), shared, coordinator, pending).await {
                debug_info(&app, &format!("Resumed analysis failed: {err}"));
            }
        });
    }
    Ok(queued)
}

#[tauri::command]
fn get_auto_analyze_new(app: AppHandle) -> bool {
    config_path(&app).map_or(true, |path| analysis_backend::read_auto_analyze_new(&path))
}

#[tauri::command]
fn set_auto_analyze_new(enabled: bool, app: AppHandle) -> Result<(), String> {
    analysis_backend::write_auto_analyze_new(&config_path(&app)?, enabled)
}

#[tauri::command]
fn get_analyze_after_import(app: AppHandle) -> bool {
    config_path(&app).map_or(false, |path| analysis_backend::read_analyze_after_import(&path))
}

#[tauri::command]
fn set_analyze_after_import(enabled: bool, app: AppHandle) -> Result<(), String> {
    analysis_backend::write_analyze_after_import(&config_path(&app)?, enabled)
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
        rerun_work(&lib, &ids)?
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

    let out = Path::new(&output_dir);
    volumes::require_removable_volume(out)?;

    sync_playlists_to_library(&*state.lock().map_err(|e| e.to_string())?, &playlists)?;
    let whole_library = pioneer_library::SyncTarget { volume: out.to_path_buf(), selection: pioneer_library::SyncSelection::WholeLibrary };
    import_plan::store_covers_for_sync(&state, &[whole_library])?;

    let lib = state.lock().map_err(|e| e.to_string())?;
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

struct TauriSyncSink(AppHandle);

impl pioneer_library::ProgressSink for TauriSyncSink {
    fn progress(&self, progress: &pioneer_library::StickProgress) {
        self.0.emit("usb-sync-progress", progress).ok();
    }

    fn finished(&self, report: &pioneer_library::StickReport) {
        self.0.emit("usb-sync-stick-report", report).ok();
    }
}

/// Sync several sticks concurrently. Emits `usb-sync-progress` per stick and
/// `usb-sync-stick-report` when a stick finishes. One failing stick does not stop
/// the others; the result carries one report per target, in order. The library
/// lock is held only while the selection is read.
#[tauri::command]
async fn sync_usb_many(
    targets: Vec<pioneer_library::SyncTarget>,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<Vec<pioneer_library::StickReport>, String> {
    let shared: SharedLibrary = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        for target in &targets {
            volumes::require_removable_volume(&target.volume)?;
        }
        let prepared = import_plan::prepare_sync_with_covers(&shared, &targets)?;
        Ok(prepared.run(&TauriSyncSink(app)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sync frontend playlists into the library.
fn sync_playlists_to_library(
    lib: &LocalLibrary,
    playlists: &[PlaylistInput],
) -> Result<(), String> {
    let playlists: Vec<models::Playlist> = playlists
        .iter()
        .map(|pl| models::Playlist {
            id: pl.id,
            name: pl.name.clone(),
            track_ids: pl.track_ids.clone(),
            parent_id: pl.parent_id,
            is_folder: pl.is_folder,
        })
        .collect();
    lib.replace_playlists(&playlists).map_err(|e| e.to_string())
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

    let cue = |hot_cue_number: u32, time_ms: u32| pioneer_library::rekordbox::ImportedCue {
        hot_cue_number,
        time_ms,
        loop_time_ms: None,
        color: None,
        name: String::new(),
    };
    for &track_id in &ids {
        if lib.get_analysis(track_id as i64).map_err(|e| e.to_string())?.is_none() {
            continue;
        }
        lib.set_cues(track_id as i64, &[cue(0, 60_000), cue(1, 75_000), cue(2, 90_000)])
            .map_err(|e| e.to_string())?;
    }

    build_track_infos(&lib)
}

/// List removable and external volumes so the frontend can offer a target-drive picker.
#[tauri::command]
fn get_mounted_volumes() -> Result<Vec<String>, String> {
    volumes::list_removable_volumes()
}

fn validated_reveal_path(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(path);
    if path.trim().is_empty() {
        return Err("No file path to reveal".to_string());
    }
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    Ok(p)
}

/// Reveal a file in the OS file manager (selects it where supported).
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    let p = validated_reveal_path(&path)?;

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(&p);
        c
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", p.display()));
        c
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(p.parent().unwrap_or(&p));
        c
    };

    command
        .spawn()
        .map_err(|e| format!("Could not open file manager: {e}"))?;
    Ok(())
}

/// Eject a mounted volume (macOS: `diskutil eject`, Linux: `udisksctl unmount`, falling back to `umount`).
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

    #[cfg(target_os = "windows")]
    {
        let _ = &path;
        return Err("Eject is not supported on Windows yet. Use Safely Remove Hardware in the taskbar.".into());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut command = match volumes::mount_device(std::path::Path::new(&path)) {
            Some(device) => {
                let mut c = std::process::Command::new("udisksctl");
                c.args(["unmount", "--block-device", &device]);
                c
            }
            None => {
                let mut c = std::process::Command::new("umount");
                c.arg(&path);
                c
            }
        };
        let output = match command.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::process::Command::new("umount")
                .arg(&path)
                .output()
                .map_err(|e| format!("Failed to run umount: {e}"))?,
            Err(e) => return Err(format!("Failed to run eject command: {e}")),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Eject failed: {stderr}"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    Ok(())
}

/// Wipe all Pioneer data from a USB drive (removes PIONEER/ and Contents/ directories).
#[tauri::command]
fn wipe_usb(path: String) -> Result<(), String> {
    let usb = std::path::Path::new(&path);
    if !usb.is_dir() {
        return Err("Not a valid directory".into());
    }
    volumes::require_removable_volume(usb)?;

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

#[derive(serde::Deserialize)]
struct RekordboxImportRequest {
    kind: rekordbox::SourceKind,
    /// `master.db` (or the folder holding it), a USB root, or a `rekordbox.xml`.
    path: String,
    #[serde(flatten)]
    options: rekordbox::ImportOptions,
}

fn load_rekordbox_source(request: &RekordboxImportRequest) -> Result<rekordbox::SourceLibrary, String> {
    let path = Path::new(&request.path);
    let loaded = match request.kind {
        rekordbox::SourceKind::MasterDb => {
            let db = if path.is_dir() { path.join("master.db") } else { path.to_path_buf() };
            rekordbox::load_masterdb(&db, None)
        }
        rekordbox::SourceKind::Usb => rekordbox::load_usb(path),
        rekordbox::SourceKind::Xml => rekordbox::load_xml(path),
    };
    loaded.map_err(|e| format!("{e:#}"))
}

/// Rekordbox's own `master.db` when it sits in the usual place on this machine.
#[tauri::command]
fn rekordbox_default_master_db() -> Option<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    [
        home.map(|h| h.join("Library/Pioneer/rekordbox/master.db")),
        appdata.map(|a| a.join("Pioneer/rekordbox/master.db")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
    .map(|p| p.to_string_lossy().to_string())
}

/// Read a Rekordbox library and compare it with the local library without changing anything.
#[tauri::command]
async fn rekordbox_import_preview(
    request: RekordboxImportRequest,
    state: State<'_, SharedLibrary>,
) -> Result<rekordbox::ImportPreview, String> {
    let shared: SharedLibrary = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let source = load_rekordbox_source(&request)?;
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.rekordbox_preview(&source, &request.options)
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Import a Rekordbox library. Emits `rekordbox-import-progress` while it runs; the
/// library lock is held only for the final write, not while ANLZ files are parsed.
#[tauri::command]
async fn rekordbox_import_run(
    request: RekordboxImportRequest,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<rekordbox::RekordboxImportReport, String> {
    let shared: SharedLibrary = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress = |p: rekordbox::ImportProgress| {
            app.emit("rekordbox-import-progress", p).ok();
        };
        progress(rekordbox::ImportProgress { stage: "reading", done: 0, total: 0 });
        let source = load_rekordbox_source(&request)?;
        let analyses = rekordbox::load_analyses(&source, &progress);
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let report = lib
            .import_rekordbox(&source, &analyses, &request.options, &progress)
            .map_err(|e| format!("{e:#}"))?;
        debug_ok(&app, &format!("Rekordbox import: {report:?}"));
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
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
        let stored = analysis_payload::read_stored_analysis(&*shared.lock().map_err(|e| e.to_string())?, track_id)?;
        Ok::<_, String>(stored.into_json())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The high-resolution 3-band series of a track for the zoomed waveform (`DetailSeries::to_bytes`),
/// measured from the audio on first use and cached in the app data folder. Sent as a binary IPC response.
#[tauri::command]
async fn get_waveform_detail(
    app: AppHandle,
    track_id: i64,
    state: State<'_, SharedLibrary>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let cache_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("waveform-detail");
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let track = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            lib.get_track(track_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Track {track_id} is not in the library"))?
        };
        waveform_detail::detail_bytes(&cache_dir, &track.source_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Measure a track's high-resolution series into the cache ahead of its zoomed waveform: below the UI's priority like
/// the analysis (the next rows), or `urgent` at the UI's (a row being pressed, opened a moment later; its
/// `get_waveform_detail` then waits for this measurement instead of starting its own). Returns nothing: the series is
/// read with `get_waveform_detail`.
#[tauri::command]
async fn prefetch_waveform_detail(
    app: AppHandle,
    track_id: i64,
    urgent: Option<bool>,
    state: State<'_, SharedLibrary>,
) -> Result<(), String> {
    let shared = state.inner().clone();
    let cache_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("waveform-detail");
    tauri::async_runtime::spawn_blocking(move || {
        let track = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            lib.get_track(track_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Track {track_id} is not in the library"))?
        };
        if urgent == Some(true) {
            return waveform_detail::detail_bytes(&cache_dir, &track.source_path).map(drop);
        }
        analysis_priority::run_below_ui(|| waveform_detail::warm_detail(&cache_dir, &track.source_path))?
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A fine stretch of the zoomed waveform (`WindowSeries::to_bytes`): `columns` columns from `start_s` to `end_s`, decoded
/// from the audio now, for zoom levels where the high-resolution series is wider than a pixel. A newer request
/// supersedes an older one, which then fails with `superseded`. Sent as a binary IPC response.
#[tauri::command]
async fn get_waveform_window(
    app: AppHandle,
    track_id: i64,
    start_s: f64,
    end_s: f64,
    columns: u32,
    state: State<'_, SharedLibrary>,
    windows: State<'_, Arc<waveform_detail::DetailWindows>>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let windows = windows.inner().clone();
    let cache_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("waveform-detail");
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let track = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            lib.get_track(track_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Track {track_id} is not in the library"))?
        };
        windows
            .window(&cache_dir, &track.source_path, start_s, end_s, columns as usize)?
            .map(|window| window.to_bytes())
            .ok_or_else(|| "superseded".to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Replace the hot and memory cues of a track (slot 0 is a memory cue, 1 to 8 are A to H).
#[tauri::command]
async fn set_cues(
    track_id: i64,
    cues: Vec<pioneer_library::rekordbox::ImportedCue>,
    state: State<'_, SharedLibrary>,
) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.set_cues(track_id, &cues).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Store an edited beat grid (nudged or set from the playhead) for an analyzed track.
#[tauri::command]
async fn set_beat_grid(
    track_id: i64,
    beats: Vec<BeatPosition>,
    offset_ms: Option<f64>,
    anchor_ms: Option<f64>,
    state: State<'_, SharedLibrary>,
) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.set_beat_grid(track_id, &beats, offset_ms, anchor_ms).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Set the BPM of an analyzed track by hand; re-analysis keeps it. The beat grid stays as it is.
#[tauri::command]
async fn set_track_bpm(track_id: i64, bpm: f64, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.set_user_bpm(track_id, bpm).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Set the Camelot key of an analyzed track by hand; re-analysis keeps it.
#[tauri::command]
async fn set_track_key(track_id: i64, key: String, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.set_user_key(track_id, &key).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Set the star rating of a track (0 clears it); it shows over the imported rating.
#[tauri::command]
async fn set_track_rating(track_id: i64, rating: u8, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.set_user_rating(track_id, rating).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Edit the title, artist, album or genre of a track in the library. File tags are never touched.
#[tauri::command]
async fn update_track_fields(
    track_id: i64,
    edits: TrackFieldEdits,
    state: State<'_, SharedLibrary>,
) -> Result<TrackFields, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let mut track = lib
            .get_track(track_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Track {track_id} is not in the library"))?;
        apply_edits(&mut track, &edits)?;
        lib.update_track(track_id, &track).map_err(|e| e.to_string())?;
        Ok(TrackFields::of(&track))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Let re-analysis replace the hand-set BPM and key of a track again.
#[tauri::command]
async fn release_user_edits(track_id: i64, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.release_user_edits(track_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Let re-analysis replace only the hand-set BPM and grid (`field` "bpm") or only the hand-set key (`field` "key").
#[tauri::command]
async fn release_user_field(track_id: i64, field: String, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        match field.as_str() {
            "bpm" => lib.release_user_bpm(track_id),
            "key" => lib.release_user_key(track_id),
            other => Err(anyhow::anyhow!("Unknown field {other:?}")),
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fit the beat grid to a new BPM (explicit, never done by `set_track_bpm`).
#[tauri::command]
async fn rescale_beat_grid(track_id: i64, bpm: f64, state: State<'_, SharedLibrary>) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        lib.rescale_beat_grid(track_id, bpm).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

const ARTWORK_THUMB_CACHE_LIMIT: usize = 400;
const ARTWORK_THUMB_JPEG_QUALITY: u8 = 85;

#[derive(Default)]
struct ArtworkThumbCache {
    entries: HashMap<(i64, u32, u64), Arc<Vec<u8>>>,
    order: VecDeque<(i64, u32, u64)>,
}

type SharedArtworkThumbs = Arc<Mutex<ArtworkThumbCache>>;

fn bytes_fingerprint(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn artwork_thumbnail(bytes: &[u8], max_px: u32) -> Vec<u8> {
    let Ok(decoded) = image::load_from_memory(bytes) else {
        return bytes.to_vec();
    };
    if decoded.width() <= max_px && decoded.height() <= max_px {
        return bytes.to_vec();
    }
    let thumb = decoded.thumbnail(max_px, max_px).to_rgb8();
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, ARTWORK_THUMB_JPEG_QUALITY);
    match thumb.write_with_encoder(encoder) {
        Ok(()) => out,
        Err(_) => bytes.to_vec(),
    }
}

fn cached_artwork_thumbnail(
    cache: &SharedArtworkThumbs,
    track_id: i64,
    bytes: &[u8],
    max_px: u32,
) -> Arc<Vec<u8>> {
    let key = (track_id, max_px, bytes_fingerprint(bytes));
    if let Some(hit) = cache.lock().ok().and_then(|c| c.entries.get(&key).cloned()) {
        return hit;
    }
    let thumb = Arc::new(artwork_thumbnail(bytes, max_px));
    if let Ok(mut c) = cache.lock() {
        if c.entries.insert(key, thumb.clone()).is_none() {
            c.order.push_back(key);
            if c.order.len() > ARTWORK_THUMB_CACHE_LIMIT {
                if let Some(oldest) = c.order.pop_front() {
                    c.entries.remove(&oldest);
                }
            }
        }
    }
    thumb
}

/// Get artwork bytes for a track from the local library DB. With `max_px` the image is
/// downscaled to fit that box (JPEG, cached in memory); without it the stored bytes are
/// returned unchanged. Sent as a binary IPC response (an `ArrayBuffer`).
#[tauri::command]
async fn get_track_artwork(
    track_id: i64,
    max_px: Option<u32>,
    state: State<'_, SharedLibrary>,
    thumbs: State<'_, SharedArtworkThumbs>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let thumbs = thumbs.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let stored = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            lib.get_artwork(track_id).map_err(|e| e.to_string())?
        };
        Ok::<_, String>(match (stored, max_px) {
            (Some(bytes), Some(px)) if px > 0 => {
                cached_artwork_thumbnail(&thumbs, track_id, &bytes, px).as_ref().clone()
            }
            (stored, _) => stored.unwrap_or_default(),
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
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

/// Row previews of the given tracks, for the list rows on screen.
#[tauri::command]
fn get_row_previews(track_ids: Vec<u32>, state: State<'_, SharedLibrary>) -> Result<Vec<dto::RowPreviewInfo>, String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    track_infos::row_preview_infos(&lib, &track_ids)
}

/// Scans every track's sidecar files again (after the list is on screen) and returns the ratings and colours that
/// changed since `load_state` served them from the labels cache.
#[tauri::command]
async fn refresh_track_labels(state: State<'_, SharedLibrary>) -> Result<Vec<dto::TrackLabelInfo>, String> {
    let reader = state.lock().map_err(|e| e.to_string())?.track_labels_reader();
    tauri::async_runtime::spawn_blocking(move || track_infos::changed_track_labels(&reader))
        .await
        .map_err(|e| e.to_string())
}

/// Load the library state. Returns all tracks and playlists.
#[tauri::command]
async fn load_state(state: State<'_, SharedLibrary>) -> Result<LoadedState, String> {
    let shared = state.inner().clone();
    perf_probe::mark("load_state_start", serde_json::json!({}));
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let state = track_infos::loaded_state(&lib)?;
        perf_probe::mark("load_state_built", serde_json::json!({ "tracks": state.tracks.len() }));
        Ok(state)
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
    let mut json = std::fs::read_to_string(&cfg)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    json["library_path"] = serde_json::Value::String(path.to_string_lossy().to_string());
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
    covers: State<'_, CoverFill>,
) -> Result<String, String> {
    let db_path = PathBuf::from(&folder_path).join("library.db");

    let new_library = LocalLibrary::open(&db_path).map_err(|e| e.to_string())?;

    // Swap the active library
    *state.lock().map_err(|e| e.to_string())? = new_library;
    spawn_stale_staging_cleanup(db_path.clone());

    // Remember the choice
    write_library_path(&app, &db_path)?;
    spawn_cover_fill(app, state.inner().clone(), covers.inner().clone());

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
    if self_check::requested() {
        self_check::run_and_exit();
    }
    perf_probe::start();
    perf_probe::install(tauri::Builder::default()) // v0.1
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            plan_import,
            commit_import,
            analyze_tracks,
            analyze_track_ids,
            resume_analysis,
            get_auto_analyze_new,
            set_auto_analyze_new,
            get_analyze_after_import,
            set_analyze_after_import,
            enqueue_analysis,
            prioritize_analysis,
            analysis_ui_busy,
            write_usb,
            sync_usb_many,
            stick_commands::get_stick,
            stick_commands::stick_add_tracks,
            stick_commands::stick_remove_tracks,
            stick_commands::stick_add_playlists,
            stick_commands::stick_remove_playlists,
            stick_commands::get_volume_capacity,
            stick_commands::plan_usb_sync,
            file_health::check_library_files,
            file_health::get_library_health,
            file_health::relocate_missing_in_folder,
            file_health::locate_track_file,
            backup_commands::get_backup_status,
            backup_commands::export_backup,
            backup_commands::read_backup,
            backup_commands::restore_backup,
            remove_tracks,
            set_test_cues,
            get_mounted_volumes,
            eject_volume,
            reveal_in_finder,
            wipe_usb,
            save_state,
            load_state,
            get_row_previews,
            refresh_track_labels,
            app_version,
            rekordbox_default_master_db,
            rekordbox_import_preview,
            rekordbox_import_run,
            read_usb_state,
            get_library_path,
            change_library_path,
            reset_library,
            analyze_track_python,
            cancel_analysis,
            get_analysis_backend,
            set_analysis_backend,
            get_analysis_data,
            get_waveform_detail,
            prefetch_waveform_detail,
            get_waveform_window,
            set_beat_grid,
            set_track_bpm,
            set_track_key,
            set_track_rating,
            update_track_fields,
            release_user_edits,
            release_user_field,
            rescale_beat_grid,
            set_cues,
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
            playback_set_tempo,
            playback_set_key_shift,
            playback_set_key_lock,
            perf_probe::perf_mark,
        ])
        .setup(|app| {
            // Open (or create) the local library database at the stored/default path
            let db_path = read_library_path(&app.handle())
                .expect("Failed to determine library database path");
            let library = LocalLibrary::open(&db_path)
                .expect("Failed to open library database");
            spawn_stale_staging_cleanup(db_path);
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                spawn_stale_part_sweep(app_data_dir.join("waveform-detail"));
            }
            let library: SharedLibrary = Arc::new(Mutex::new(library));
            let health = file_health::SharedHealth::default();
            app.manage(library.clone());
            app.manage(health.clone());
            file_health::spawn_check(app.handle().clone(), library.clone(), health);
            let covers = CoverFill::default();
            app.manage(covers.clone());
            spawn_cover_fill(app.handle().clone(), library, covers);
            app.manage(Arc::new(ImportPlans::default()));
            app.manage(AnalysisCoordinator::default());
            app.manage(SharedArtworkThumbs::default());
            app.manage(Arc::new(waveform_detail::DetailWindows::default()));

            match PlaybackEngine::new() {
                Ok(playback) => {
                    app.manage(Arc::new(playback));
                }
                Err(e) => eprintln!("audio playback unavailable: {e}"),
            }

            focus_main_window(&app.handle());
            perf_probe::mark("setup_done", serde_json::json!({}));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Ready => {
                    perf_probe::mark("ready", serde_json::json!({}));
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
    use super::{artwork_thumbnail, validated_reveal_path};
    use crate::analysis_python::{parse_analyzer_stdout, python_result_to_analysis};

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 90]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).expect("encode");
        out.into_inner()
    }

    #[test]
    fn reveal_path_rejects_empty_and_missing_paths() {
        assert!(validated_reveal_path("  ").is_err());
        let err = validated_reveal_path("/definitely/not/here.mp3").unwrap_err();
        assert!(err.contains("File not found"));
    }

    #[test]
    fn reveal_path_accepts_existing_paths() {
        let dir = std::env::temp_dir();
        assert_eq!(validated_reveal_path(dir.to_str().unwrap()).unwrap(), dir);
    }

    #[test]
    fn artwork_thumbnail_fits_the_box_and_stays_a_decodable_jpeg() {
        let thumb = artwork_thumbnail(&png_bytes(1200, 800), 512);
        let decoded = image::load_from_memory(&thumb).expect("decode");
        assert_eq!(image::guess_format(&thumb).expect("format"), image::ImageFormat::Jpeg);
        assert_eq!((decoded.width(), decoded.height()), (512, 341));
    }

    #[test]
    fn artwork_thumbnail_keeps_small_images_and_garbage_untouched() {
        let small = png_bytes(100, 80);
        assert_eq!(artwork_thumbnail(&small, 512), small);
        assert_eq!(artwork_thumbnail(b"not an image", 512), b"not an image".to_vec());
    }

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
