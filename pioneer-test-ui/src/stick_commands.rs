use std::path::Path;

use pioneer_library::sticks;
use pioneer_library::{LocalLibrary, StickContents, SyncTarget, UsbSyncPlan, VolumeCapacity};
use tauri::State;

use crate::analysis_run::SharedLibrary;

fn volume_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn known_stick(path: &Path, lib: &LocalLibrary) -> Result<StickContents, String> {
    match sticks::read_stick_id(path).map_err(|e| format!("{e:#}"))? {
        Some(id) => lib.stick_contents(&id).map_err(|e| e.to_string()),
        None => Ok(StickContents::default()),
    }
}

fn claimed_stick(path: &Path, lib: &LocalLibrary) -> Result<String, String> {
    if !path.is_dir() {
        return Err(format!("{} is not mounted.", path.display()));
    }
    let id = sticks::ensure_stick_id(path).map_err(|e| format!("{e:#}"))?;
    lib.remember_stick(&id, &volume_name(path), path).map_err(|e| e.to_string())?;
    Ok(id)
}

fn edit_stick(
    path: &str,
    state: &State<'_, SharedLibrary>,
    edit: impl FnOnce(&LocalLibrary, &str) -> anyhow::Result<usize>,
) -> Result<(usize, StickContents), String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    let path = Path::new(path);
    let id = claimed_stick(path, &lib)?;
    let changed = edit(&lib, &id).map_err(|e| e.to_string())?;
    Ok((changed, lib.stick_contents(&id).map_err(|e| e.to_string())?))
}

/// What the library remembers for the stick at `path`; empty for a stick fourfour never touched.
#[tauri::command]
pub fn get_stick(path: String, state: State<'_, SharedLibrary>) -> Result<StickContents, String> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    known_stick(Path::new(&path), &lib)
}

/// Adds tracks to the stick's "All tracks". Gives the stick an id on first use.
#[tauri::command]
pub fn stick_add_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), String> {
    edit_stick(&path, &state, |lib, id| lib.add_stick_tracks(id, &track_ids))
}

#[tauri::command]
pub fn stick_remove_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), String> {
    edit_stick(&path, &state, |lib, id| lib.remove_stick_tracks(id, &track_ids))
}

#[tauri::command]
pub fn stick_add_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), String> {
    edit_stick(&path, &state, |lib, id| lib.add_stick_playlists(id, &playlist_ids))
}

#[tauri::command]
pub fn stick_remove_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), String> {
    edit_stick(&path, &state, |lib, id| lib.remove_stick_playlists(id, &playlist_ids))
}

#[tauri::command]
pub fn get_volume_capacity(path: String) -> Result<VolumeCapacity, String> {
    pioneer_library::volume_capacity(Path::new(&path)).map_err(|e| format!("{e:#}"))
}

/// Dry run of a sync: what `sync_usb_many` with the same target would do. Writes nothing.
#[tauri::command]
pub async fn plan_usb_sync(
    target: SyncTarget,
    state: State<'_, SharedLibrary>,
) -> Result<UsbSyncPlan, String> {
    let shared: SharedLibrary = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = {
            let lib = shared.lock().map_err(|e| e.to_string())?;
            lib.prepare_multi_sync(&[target]).map_err(|e| format!("{e:#}"))?
        };
        prepared
            .plan()
            .into_iter()
            .next()
            .ok_or_else(|| "No USB stick selected.".to_string())?
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
