#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis_backend;
mod analysis_failures;
mod analysis_order;
mod analysis_payload;
mod analysis_priority;
mod analysis_run;
mod artwork_thumbs;
mod app_log;
mod backup_commands;
mod band_analysis;
mod command_error;
mod data_migration;
mod dto;
mod early_state;
mod file_health;
mod heap;
mod import_plan;
mod perf_probe;
mod playback;
mod recovery_commands;
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
mod stick_busy;
mod stick_commands;
mod tag_scan;
mod volumes;
mod waveform_detail;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use pioneer_library::rekordbox;
use pioneer_library::{BeatPosition, LocalLibrary};
use pioneer_usb_writer::models;

use analysis_backend::BackendEnv;
use analysis_failures::FailureMemo;
use artwork_thumbs::{BATCH_THUMBNAIL_THREADS, cached_artwork_thumbnail, stored_covers, thumbnail_batch, SharedArtworkThumbs};
use command_error::{CommandError, ErrorKind};
use analysis_run::{
    merge_into_analysis_queue, rerun_work, resolve_analysis_work, without_known_failures, AnalysisCoordinator, AnalysisSink,
    AnalysisWorkItem, LogLevel, RunOutcome, SharedLibrary,
};
use track_fields::{apply_edits, TrackFieldEdits, TrackFields};
use track_infos::build_track_infos;
use import_plan::{ImportPlan, ImportPlans};
use dto::{PlaylistInput, ProgressPayload, TrackInfo};
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
    app_log::write(level, message);
    app.emit("debug-log", serde_json::json!({ "level": level, "message": message })).ok();
}

fn debug_info(app: &AppHandle, msg: &str)  { debug_log(app, "info", msg); }
fn debug_ok(app: &AppHandle, msg: &str)    { debug_log(app, "ok", msg); }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Adds the tracks not yet in the library; their covers follow from `spawn_cover_fill`, so the
/// rows come back without artwork and `import-covers` names the ones that got a cover.
fn add_scanned_tracks(lib: &LocalLibrary, scanned: Vec<models::Track>) -> Result<Vec<TrackInfo>, CommandError> {
    if scanned.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = lib.track_paths().map_err(CommandError::from_anyhow)?;
    let fresh: Vec<models::Track> = scanned.into_iter().filter(|t| seen.insert(t.source_path.clone())).collect();
    let ids = lib.add_tracks_covers_later(&fresh).map_err(CommandError::from_anyhow)?;
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
    std::thread::spawn(move || {
        move_legacy_covers(&app, &library);
        fill_pending_covers(&app, &library, &fill);
    });
}

/// Moves the covers a v7 library kept in its database into the cover store, a batch at a time with the library
/// lock let go in between. Stops (covers stay readable in the database) when the volume lacks the space or a
/// batch fails; the next launch tries again.
fn move_legacy_covers(app: &AppHandle, library: &SharedLibrary) {
    let check = library
        .lock()
        .map_err(|e| e.to_string())
        .and_then(|lib| lib.check_space_for_legacy_covers(&pioneer_library::statvfs_available_bytes).map_err(|e| format!("{e:#}")));
    if let Err(e) = check {
        debug_log(app, "error", &e);
        return;
    }
    let started = std::time::Instant::now();
    let mut moved = 0;
    loop {
        let step = library
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|lib| lib.move_legacy_covers(LEGACY_COVER_BATCH).map_err(|e| format!("{e:#}")));
        match step {
            Ok(step) => {
                moved += step.moved;
                if step.remaining == 0 {
                    break;
                }
            }
            Err(e) => {
                debug_log(app, "error", &format!("Moving covers out of the library failed: {e}"));
                return;
            }
        }
    }
    if moved > 0 {
        debug_log(app, "info", &format!("Moved {moved} covers out of the library database in {:.1} s", started.elapsed().as_secs_f64()));
    }
}

const LEGACY_COVER_BATCH: usize = 32;

fn fill_pending_covers(app: &AppHandle, library: &SharedLibrary, fill: &CoverFill) {
    use std::sync::atomic::Ordering;
    loop {
        let mut unsent: Vec<u32> = Vec::new();
        let mut last_emit = std::time::Instant::now();
        let result = import_plan::store_pending_covers(library, None, &mut |ids| {
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
            debug_log(app, "error", &format!("Storing imported covers failed: {e}"));
            return;
        }
        let more = library.lock().ok().and_then(|lib| lib.pending_artwork(0, 1).ok()).is_some_and(|p| !p.is_empty());
        if !more || fill.0.swap(true, Ordering::AcqRel) {
            return;
        }
    }
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
) -> Result<ImportPlan, CommandError> {
    let shared = state.inner().clone();
    let plans = plans.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let known = shared.lock().map_err(|e| e.to_string())?.track_paths().map_err(CommandError::from_anyhow)?;
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

/// The audio files under each dropped path, without reading tags: fast enough to lay out
/// playlists for a drop before `plan_import` finishes.
#[tauri::command]
async fn survey_import(paths: Vec<String>) -> Result<Vec<import_plan::DroppedPath>, CommandError> {
    Ok(tauri::async_runtime::spawn_blocking(move || import_plan::dropped_audio(&paths)).await.map_err(|e| e.to_string())?)
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
) -> Result<ImportCommit, CommandError> {
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
) -> Result<(), CommandError> {
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
    analysis_run::prioritize_track(shared, coordinator.inner().clone(), item, sink, env)
        .await
        .map_err(analysis_start_error)
}

/// Append or prepend tracks to the active analysis run (no-op when idle).
#[tauri::command(async)]
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

/// The tracks on screen, most wanted first: queued ones among them are analysed next (see `AnalysisCoordinator::focus`).
#[tauri::command(async)]
fn focus_analysis(ids: Vec<i64>, coordinator: State<'_, AnalysisCoordinator>) {
    coordinator.focus(&ids);
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

fn analysis_coordinator(app: &AppHandle) -> AnalysisCoordinator {
    match app.path().app_data_dir() {
        Ok(dir) => {
            let models = fourfour_analysis::service::locate_models_in(Some(&dir), app.path().resource_dir().ok().as_deref());
            let key = fourfour_analysis::service::analysis_key(models.as_deref());
            AnalysisCoordinator::default().with_failure_memo(FailureMemo::in_dir(dir, &key))
        }
        Err(_) => AnalysisCoordinator::default(),
    }
}

fn analysis_backend_env(app: &AppHandle) -> BackendEnv {
    BackendEnv::from_process(app.path().app_data_dir().ok(), app.path().resource_dir().ok())
}

fn all_tracks_failed<T: std::fmt::Display>(total: T) -> CommandError {
    CommandError::new(ErrorKind::AnalysisFailed, format!("Analysis failed for all {total} tracks — see debug log"))
}

fn analysis_start_error(message: String) -> CommandError {
    let kind = if message == analysis_backend::NO_BACKEND_ERROR { ErrorKind::ModelsMissing } else { ErrorKind::Other };
    CommandError::new(kind, message)
}

/// Run analysis for the given tracks with the analyzer. Used by bulk and per-selection commands.
async fn run_track_analysis(
    app: AppHandle,
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    pending: Vec<AnalysisWorkItem>,
) -> Result<Vec<TrackInfo>, CommandError> {
    let sink = Arc::new(TauriAnalysisSink(app.clone()));
    let env = analysis_backend_env(&app);
    let outcome = analysis_run::run_track_analysis(shared.clone(), coordinator, pending, sink, env)
        .await
        .map_err(analysis_start_error)?;
    if let RunOutcome::Finished { errors, total } = outcome {
        if errors > 0 && errors == total {
            return Err(all_tracks_failed(total));
        }
    }
    let lib = shared.lock().map_err(|e| e.to_string())?;
    Ok(build_track_infos(&lib)?)
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

/// Analyze all tracks that have not yet been analyzed.
#[tauri::command]
async fn analyze_tracks(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    coordinator: State<'_, AnalysisCoordinator>,
) -> Result<Vec<TrackInfo>, CommandError> {
    let shared: SharedLibrary = state.inner().clone();
    let pending: Vec<AnalysisWorkItem> = {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let unanalyzed_ids = lib.get_unanalyzed_track_ids().map_err(CommandError::from_anyhow)?;
        let work = resolve_analysis_work(&lib, &unanalyzed_ids)?;
        without_known_failures(&lib, &coordinator, work)?.0
    };
    run_track_analysis(app, shared, coordinator.inner().clone(), pending).await
}

/// Queue tracks left with missing stages (and, with the setting on, never-analysed ones) at launch.
/// Returns how many tracks were queued; the run continues in the background.
#[tauri::command(async)]
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
        let work = analysis_run::resume_work_in_saved_order(&lib, include_new, app_data_dir.as_deref())?;
        let (work, skipped) = without_known_failures(&lib, &coordinator, work)?;
        if skipped > 0 {
            debug_info(&app, &format!("Resume left out {skipped} tracks that failed analysis twice on an unchanged file"));
        }
        work
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
) -> Result<Vec<TrackInfo>, CommandError> {
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
#[tauri::command(async)]
fn write_usb(
    output_dir: String,
    playlists: Vec<PlaylistInput>,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    busy: State<'_, stick_busy::BusySticks>,
) -> Result<models::SyncReport, CommandError> {
    debug_info(&app, &format!("USB sync → {output_dir}"));

    let out = Path::new(&output_dir);
    let _busy = busy.acquire(out)?;
    volumes::require_removable_volume(out)?;

    sync_playlists_to_library(&*state.lock().map_err(|e| e.to_string())?, &playlists)?;
    let whole_library = pioneer_library::SyncTarget { volume: out.to_path_buf(), selection: pioneer_library::SyncSelection::WholeLibrary };
    import_plan::store_covers_for_sync(&state, &[whole_library])?;

    let lib = state.lock().map_err(|e| e.to_string())?;
    let report = lib.sync_usb(out).map_err(|e| CommandError::from_anyhow(e).for_stick())?;

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
        self.0.emit("usb-sync-stick-report", stick_commands::StickReportWire::from(report.clone())).ok();
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
    busy: State<'_, stick_busy::BusySticks>,
) -> Result<Vec<stick_commands::StickReportWire>, CommandError> {
    let shared: SharedLibrary = state.inner().clone();
    let guards = busy.acquire_all(targets.iter().map(|target| target.volume.as_path()))?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guards = guards;
        for target in &targets {
            volumes::require_removable_volume(&target.volume)?;
        }
        let prepared = import_plan::prepare_sync_with_covers(&shared, &targets)?;
        Ok(prepared.run(&TauriSyncSink(app)).into_iter().map(stick_commands::StickReportWire::from).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sync frontend playlists into the library.
fn sync_playlists_to_library(
    lib: &LocalLibrary,
    playlists: &[PlaylistInput],
) -> Result<(), CommandError> {
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
    lib.replace_playlists(&playlists).map_err(CommandError::from_anyhow)
}

/// Remove tracks by ID from the library.
#[tauri::command(async)]
fn remove_tracks(ids: Vec<u32>, state: State<'_, SharedLibrary>) -> Result<(), CommandError> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    let ids_i64: Vec<i64> = ids.into_iter().map(|id| id as i64).collect();
    lib.remove_tracks(&ids_i64).map_err(CommandError::from_anyhow)
}

/// Set 2 test hot cues (A at 1:00, B at 1:30) on the selected tracks.
/// Replaces any existing cues on those tracks.
#[tauri::command(async)]
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
#[tauri::command(async)]
fn get_mounted_volumes() -> Result<Vec<String>, String> {
    volumes::list_removable_volumes()
}

fn validated_reveal_path(path: &str) -> Result<std::path::PathBuf, CommandError> {
    let p = std::path::PathBuf::from(path);
    if path.trim().is_empty() {
        return Err(CommandError::other("No file path to reveal"));
    }
    if !p.exists() {
        return Err(CommandError::new(ErrorKind::NotFound, format!("File not found: {path}")).with_path(&p));
    }
    Ok(p)
}

/// Reveal a file in the OS file manager (selects it where supported).
#[tauri::command(async)]
fn reveal_in_finder(path: String) -> Result<(), CommandError> {
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
        .map_err(|e| CommandError::from_io(&e, format!("Could not open file manager: {e}")))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn eject_failure(stderr: &str) -> CommandError {
    let lowered = stderr.to_lowercase();
    let busy = ["busy", "in use", "dissented"].iter().any(|word| lowered.contains(word));
    CommandError::new(if busy { ErrorKind::StickBusy } else { ErrorKind::Other }, format!("Eject failed: {stderr}"))
}

/// Eject a mounted volume (macOS: `diskutil eject`, Linux: `udisksctl unmount`, falling back to `umount`).
#[tauri::command(async)]
fn eject_volume(path: String, busy: State<'_, stick_busy::BusySticks>) -> Result<(), CommandError> {
    let _busy = busy.acquire(Path::new(&path))?;
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("diskutil")
            .args(["eject", &path])
            .output()
            .map_err(|e| CommandError::from_io(&e, format!("Failed to run diskutil: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eject_failure(&stderr));
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
                .map_err(|e| CommandError::from_io(&e, format!("Failed to run umount: {e}")))?,
            Err(e) => return Err(CommandError::from_io(&e, format!("Failed to run eject command: {e}"))),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eject_failure(&stderr));
        }
    }

    #[cfg(not(target_os = "windows"))]
    Ok(())
}

/// Wipe all Pioneer data from a USB drive (removes PIONEER/ and Contents/ directories).
#[tauri::command(async)]
fn wipe_usb(path: String, busy: State<'_, stick_busy::BusySticks>) -> Result<(), CommandError> {
    let usb = std::path::Path::new(&path);
    let _busy = busy.acquire(usb)?;
    if !usb.is_dir() {
        return Err(CommandError::new(ErrorKind::DriveGone, "Not a valid directory").with_path(usb));
    }
    volumes::require_removable_volume(usb)?;

    let pioneer_dir = usb.join("PIONEER");
    let contents_dir = usb.join("Contents");

    if pioneer_dir.exists() {
        std::fs::remove_dir_all(&pioneer_dir)
            .map_err(|e| CommandError::from_io(&e, format!("Failed to remove PIONEER/: {e}")).with_path(&pioneer_dir).for_stick())?;
    }
    if contents_dir.exists() {
        std::fs::remove_dir_all(&contents_dir)
            .map_err(|e| CommandError::from_io(&e, format!("Failed to remove Contents/: {e}")).with_path(&contents_dir).for_stick())?;
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
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get stored analysis data for a track from the local library DB, as `StoredAnalysis::into_wire` bytes (a binary IPC
/// response: the JSON head and the band bytes after it, no base64).
#[tauri::command]
async fn get_analysis_data(
    track_id: i64,
    state: State<'_, SharedLibrary>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let stored = analysis_payload::read_stored_analysis(&*shared.lock().map_err(|e| e.to_string())?, track_id)?;
        Ok::<_, String>(stored.into_wire())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
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

/// Thumbnails of many tracks in one call, for the list rows on screen: one binary response in the layout of
/// `artwork_thumbs::thumbnail_batch`, each entry the same bytes `get_track_artwork` sends for that track and `max_px`.
#[tauri::command]
async fn get_track_artworks(
    track_ids: Vec<i64>,
    max_px: u32,
    state: State<'_, SharedLibrary>,
    thumbs: State<'_, SharedArtworkThumbs>,
) -> Result<tauri::ipc::Response, String> {
    let shared = state.inner().clone();
    let thumbs = thumbs.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let stored = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            stored_covers(&lib, &track_ids)?
        };
        Ok::<_, String>(thumbnail_batch(&thumbs, &track_ids, &stored, max_px, BATCH_THUMBNAIL_THREADS))
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
#[tauri::command(async)]
fn read_usb_state(path: String) -> Result<Option<UsbStateResponse>, CommandError> {
    let state = pioneer_usb_writer::reader::read_usb_state(Path::new(&path))
        .map_err(|e| CommandError::from_anyhow(e).with_path(&path).for_stick())?;

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
#[tauri::command(async)]
fn save_state(
    playlists: Vec<PlaylistInput>,
    _app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<(), CommandError> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    sync_playlists_to_library(&lib, &playlists)
}

/// Row previews of the given tracks, for the list rows on screen.
#[tauri::command(async)]
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

/// Load the library state. Returns all tracks and playlists as one binary buffer (`LoadedState::to_bytes`).
#[tauri::command]
async fn load_state(app: AppHandle) -> Result<tauri::ipc::Response, CommandError> {
    let Some(state) = app.try_state::<SharedLibrary>() else {
        return Err(app
            .try_state::<LibraryOpenFailure>()
            .map(|failure| failure.0.clone())
            .unwrap_or_else(|| CommandError::new(ErrorKind::LibraryNotOpen, "The library is not open")));
    };
    let shared = state.inner().clone();
    let early = app.try_state::<Arc<early_state::EarlyState>>().map(|early| early.inner().clone());
    perf_probe::mark("load_state_start", serde_json::json!({}));
    tauri::async_runtime::spawn_blocking(move || {
        let lib = shared.lock().map_err(|e| e.to_string())?;
        let (bytes, was_early) = match early {
            Some(early) => early_state::encoded_state(&lib, &early)?,
            None => (track_infos::loaded_state(&lib)?.to_bytes()?, false),
        };
        perf_probe::mark("load_state_built", serde_json::json!({ "early": was_early }));
        perf_probe::mark("load_state_encoded", serde_json::json!({ "bytes": bytes.len() }));
        Ok(tauri::ipc::Response::new(bytes))
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
#[tauri::command(async)]
fn change_library_path(
    folder_path: String,
    app: AppHandle,
    state: State<'_, SharedLibrary>,
    covers: State<'_, CoverFill>,
) -> Result<String, CommandError> {
    let db_path = PathBuf::from(&folder_path).join("library.db");

    let new_library = LocalLibrary::open(&db_path).map_err(|e| CommandError::from_anyhow(e).with_path(&db_path))?;

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
#[tauri::command(async)]
fn reset_library(
    app: AppHandle,
    state: State<'_, SharedLibrary>,
) -> Result<String, CommandError> {
    let db_path = read_library_path(&app)?;

    // Release file handles before deleting SQLite files.
    {
        let mut lib = state.lock().map_err(|e| e.to_string())?;
        *lib = LocalLibrary::open_in_memory().map_err(|e| e.to_string())?;
    }

    remove_library_files(&db_path)?;

    let fresh = LocalLibrary::open(&db_path).map_err(|e| CommandError::from_anyhow(e).with_path(&db_path))?;
    {
        let mut lib = state.lock().map_err(|e| e.to_string())?;
        *lib = fresh;
    }

    Ok(db_path.to_string_lossy().to_string())
}

/// Remove SQLite library files and the sibling `anlz/` analysis cache and `covers/` cover store directories.
fn remove_library_files(db_path: &Path) -> Result<(), CommandError> {
    let base = db_path.to_string_lossy();
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{base}{suffix}"));
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| CommandError::from_io(&e, format!("Failed to remove {}: {e}", path.display())).with_path(&path))?;
        }
    }

    if let Some(parent) = db_path.parent() {
        for folder in [parent.join("anlz"), pioneer_library::covers_dir_of(db_path)] {
            if folder.exists() {
                std::fs::remove_dir_all(&folder)
                    .map_err(|e| CommandError::from_io(&e, format!("Failed to remove {}: {e}", folder.display())).with_path(&folder))?;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

struct LibraryOpenFailure(CommandError);

/// The library opened (and its `load_state` answer built) before the webview window exists; the setup takes it.
struct StartupLibrary(Mutex<Option<std::sync::mpsc::Receiver<Result<early_state::OpenedLibrary, CommandError>>>>);

impl StartupLibrary {
    fn take(&self) -> std::sync::mpsc::Receiver<Result<early_state::OpenedLibrary, CommandError>> {
        self.0.lock().expect("startup library lock").take().expect("the startup library was already taken")
    }
}

/// Starts opening the library and building the `load_state` answer. Needs the app handle for the data directory,
/// so it runs right after `build`, before `run` creates the window.
fn start_library(app: &tauri::App) {
    let early = Arc::new(early_state::EarlyState::default());
    app.manage(early.clone());
    let handle = app.handle().clone();
    let opened = early_state::open_and_prepare(
        move || {
            let opened = open_startup_library(read_library_path(&handle));
            perf_probe::mark("library_opened", serde_json::json!({}));
            opened
        },
        early,
        |built| match built {
            Ok(()) => perf_probe::mark("early_state_ready", serde_json::json!({})),
            Err(e) => app_log::warn(&format!("early library state not built: {e}")),
        },
    );
    app.manage(StartupLibrary(Mutex::new(Some(opened))));
}

fn open_startup_library(db_path: Result<PathBuf, String>) -> Result<(PathBuf, LocalLibrary), CommandError> {
    let db_path = db_path.map_err(|e| CommandError::new(ErrorKind::LibraryNotOpen, format!("Couldn't find where the library is stored: {e}")))?;
    let library = LocalLibrary::open(&db_path).map_err(|e| {
        let message = format!("Couldn't open the library at {}: {e:#}", db_path.display());
        CommandError::from_anyhow_chain(e).with_message(message).with_path(&db_path)
    })?;
    Ok((db_path, library))
}

fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        app_log::warn("focus_main_window: no webview labeled \"main\"");
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn main() {
    heap::limit_arenas();
    if self_check::requested() {
        self_check::run_and_exit();
    }
    let context = tauri::generate_context!();
    let identifier = context.config().identifier.clone();
    let migration = data_migration::migrate_legacy_data(&identifier, data_migration::Os::current(), &|name| std::env::var_os(name));
    app_log::start_early(&identifier);
    data_migration::log_migration(&migration);
    perf_probe::start();
    let app = perf_probe::install(tauri::Builder::default()) // v0.1
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            plan_import,
            survey_import,
            commit_import,
            analyze_tracks,
            analyze_track_ids,
            resume_analysis,
            get_auto_analyze_new,
            set_auto_analyze_new,
            get_analyze_after_import,
            set_analyze_after_import,
            enqueue_analysis,
            focus_analysis,
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
            app_log::log_directory,
            app_log::log_frontend,
            rekordbox_default_master_db,
            rekordbox_import_preview,
            rekordbox_import_run,
            read_usb_state,
            get_library_path,
            change_library_path,
            reset_library,
            recovery_commands::recover_retry_library,
            recovery_commands::recover_open_library,
            recovery_commands::recover_new_library,
            recovery_commands::recover_restore_backup,
            cancel_analysis,
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
            get_track_artworks,
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
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                spawn_stale_part_sweep(app_data_dir.join("waveform-detail"));
            }
            let health = file_health::SharedHealth::default();
            app.manage(health.clone());
            app.manage(recovery_commands::RecoveryGate::default());
            let covers = CoverFill::default();
            app.manage(covers.clone());
            let startup = app.state::<StartupLibrary>().take();
            match startup.recv().expect("the library open thread stopped without a result") {
                Ok((db_path, library)) => {
                    spawn_stale_staging_cleanup(db_path);
                    app.manage(library.clone());
                    file_health::spawn_check(app.handle().clone(), library.clone(), health);
                    spawn_cover_fill(app.handle().clone(), library, covers);
                }
                Err(message) => {
                    app_log::error(&message.to_string());
                    app.manage(LibraryOpenFailure(message));
                }
            }
            app.manage(Arc::new(ImportPlans::default()));
            app.manage(stick_busy::BusySticks::default());
            app.manage(analysis_coordinator(app.handle()));
            app.manage(SharedArtworkThumbs::default());
            app.manage(Arc::new(waveform_detail::DetailWindows::default()));

            match PlaybackEngine::new() {
                Ok(playback) => {
                    app.manage(Arc::new(playback));
                }
                Err(e) => app_log::warn(&format!("audio playback unavailable: {e}")),
            }

            focus_main_window(&app.handle());
            perf_probe::mark("setup_done", serde_json::json!({}));
            Ok(())
        })
        .build(context)
        .expect("error while building tauri application");
    start_library(&app);
    app.run(|app_handle, event| {
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
    use super::artwork_thumbs::artwork_thumbnail;
    use super::{analysis_start_error, eject_failure, validated_reveal_path};
    use crate::analysis_backend;
    use crate::command_error::{CommandError, ErrorKind};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn assert_off_main_thread(source: &str, commands: &[&str]) {
        for name in commands {
            let signature = source
                .find(&format!("fn {name}("))
                .unwrap_or_else(|| panic!("command {name} not found"));
            let head = &source[..signature];
            let attribute = &head[head.rfind("#[tauri::command").expect("tauri attribute")..];
            assert!(
                attribute.starts_with("#[tauri::command(async)]") || attribute.trim_end().ends_with("async"),
                "{name} would run on the main thread"
            );
        }
    }

    #[test]
    fn volume_and_stick_commands_run_off_the_main_thread() {
        assert_off_main_thread(
            include_str!("main.rs"),
            &[
                "get_mounted_volumes",
                "eject_volume",
                "enqueue_analysis",
                "focus_analysis",
                "resume_analysis",
                "remove_tracks",
                "set_test_cues",
                "reveal_in_finder",
                "read_usb_state",
                "get_row_previews",
                "save_state",
                "change_library_path",
                "reset_library",
                "write_usb",
                "wipe_usb",
            ],
        );
        assert_off_main_thread(include_str!("backup_commands.rs"), &["read_backup"]);
        assert_off_main_thread(include_str!("file_health.rs"), &["locate_track_file"]);
        assert_off_main_thread(
            include_str!("stick_commands.rs"),
            &["get_stick", "stick_add_tracks", "stick_remove_tracks", "stick_add_playlists", "stick_remove_playlists", "get_volume_capacity"],
        );
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 90]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).expect("encode");
        out.into_inner()
    }

    struct ScratchDir(std::path::PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("ff-startup-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn startup_with_a_damaged_library_is_an_error_and_leaves_the_file_alone() {
        let dir = ScratchDir::new("damaged");
        let db = dir.path().join("library.db");
        let damaged = b"SQLite format 3\0 but the rest is junk".repeat(200);
        std::fs::write(&db, &damaged).unwrap();
        let outcome = std::panic::catch_unwind(|| super::open_startup_library(Ok(db.clone())).map(|_| ()));
        let error = outcome.expect("startup must not panic").unwrap_err();
        assert!(error.message.starts_with("Couldn't open the library at "), "{error}");
        assert!(error.message.contains("library.db"), "{error}");
        assert_eq!(error.kind, ErrorKind::LibraryDamaged);
        assert_eq!(error.path.as_deref(), db.to_str());
        assert_eq!(std::fs::read(&db).unwrap(), damaged);
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("library.db")], "no side files created or removed");
    }

    #[test]
    fn startup_without_a_library_path_is_an_error() {
        let error = super::open_startup_library(Err("no app data dir".into())).map(|_| ()).unwrap_err();
        assert!(error.message.contains("no app data dir"), "{error}");
        assert_eq!(error.kind, ErrorKind::LibraryNotOpen);
        assert_eq!(error.path, None);
    }

    fn open_on_the_startup_thread(db_path: Result<PathBuf, String>) -> Result<(PathBuf, super::SharedLibrary), CommandError> {
        let early = Arc::new(super::early_state::EarlyState::default());
        super::early_state::open_and_prepare(move || super::open_startup_library(db_path), early, |_| {})
            .recv()
            .expect("the open thread sends a result")
    }

    #[test]
    fn the_startup_thread_reports_a_damaged_library_exactly_like_the_direct_open() {
        let dir = ScratchDir::new("damaged-thread");
        let db = dir.path().join("library.db");
        let damaged = b"SQLite format 3\0 but the rest is junk".repeat(200);
        std::fs::write(&db, &damaged).unwrap();
        let direct = super::open_startup_library(Ok(db.clone())).map(|_| ()).unwrap_err();
        let threaded = open_on_the_startup_thread(Ok(db.clone())).map(|_| ()).unwrap_err();
        assert_eq!(threaded, direct);
        assert_eq!(threaded.kind, ErrorKind::LibraryDamaged);
        assert_eq!(std::fs::read(&db).unwrap(), damaged);
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("library.db")]);
    }

    #[test]
    fn the_startup_thread_reports_a_missing_library_path_exactly_like_the_direct_open() {
        let direct = super::open_startup_library(Err("no app data dir".into())).map(|_| ()).unwrap_err();
        let threaded = open_on_the_startup_thread(Err("no app data dir".into())).map(|_| ()).unwrap_err();
        assert_eq!(threaded, direct);
    }

    #[test]
    fn the_startup_thread_creates_a_missing_library_file_and_answers_load_state_from_it() {
        let dir = ScratchDir::new("missing-thread");
        let db = dir.path().join("library.db");
        let early = Arc::new(super::early_state::EarlyState::default());
        let opened = super::early_state::open_and_prepare(
            {
                let db = db.clone();
                move || super::open_startup_library(Ok(db))
            },
            early.clone(),
            |_| {},
        );
        let (path, library) = opened.recv().unwrap().unwrap();
        assert_eq!(path, db);
        let lib = library.lock().unwrap();
        let (bytes, was_early) = super::early_state::encoded_state(&lib, &early).unwrap();
        assert!(was_early);
        assert_eq!(bytes, super::track_infos::loaded_state(&lib).unwrap().to_bytes().unwrap());
    }

    #[test]
    fn startup_with_a_healthy_library_opens_it() {
        let dir = ScratchDir::new("healthy");
        let (path, library) = super::open_startup_library(Ok(dir.path().join("library.db"))).unwrap();
        assert_eq!(path, dir.path().join("library.db"));
        assert!(library.get_all_tracks().unwrap().is_empty());
    }

    #[test]
    fn reveal_path_rejects_empty_and_missing_paths() {
        assert!(validated_reveal_path("  ").is_err());
        let err = validated_reveal_path("/definitely/not/here.mp3").unwrap_err();
        assert!(err.message.contains("File not found"));
        assert_eq!(err.kind, ErrorKind::NotFound);
        assert_eq!(err.path.as_deref(), Some("/definitely/not/here.mp3"));
    }

    #[test]
    fn a_missing_analysis_backend_is_models_missing_and_other_start_errors_are_other() {
        let missing = analysis_start_error(analysis_backend::NO_BACKEND_ERROR.to_string());
        assert_eq!(missing.kind, ErrorKind::ModelsMissing);
        assert_eq!(missing.message, analysis_backend::NO_BACKEND_ERROR);
        assert_eq!(analysis_start_error("something else".to_string()).kind, ErrorKind::Other);
    }

    #[test]
    fn a_run_where_every_track_failed_is_analysis_failed_with_the_old_text() {
        let error = super::all_tracks_failed(4);
        assert_eq!(error.kind, ErrorKind::AnalysisFailed);
        assert_eq!(error.message, "Analysis failed for all 4 tracks — see debug log");
    }

    #[test]
    fn eject_output_that_says_busy_is_stick_busy() {
        for stderr in ["Error unmounting: target is busy", "umount: /media/x: device is in use", "Volume failed to eject: dissented by PID 12"] {
            assert_eq!(eject_failure(stderr).kind, ErrorKind::StickBusy, "{stderr}");
        }
        let other = eject_failure("Volume does not exist");
        assert_eq!(other.kind, ErrorKind::Other);
        assert_eq!(other.message, "Eject failed: Volume does not exist");
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
}
