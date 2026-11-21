use std::path::{Path, PathBuf};

use pioneer_library::{
    FolderRelocation, ReconnectPlan, RemapApplied, RemapPreview, RemapUndone, Relinked, fingerprint_files, plan_reconnect,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analysis_run::SharedLibrary;
use crate::file_health::{LibraryHealth, SharedHealth, lock, mark_relinked, run_check};

const FINGERPRINT_CHUNK: usize = 500;

#[derive(Debug, Clone, Deserialize)]
pub struct ReconnectPick {
    pub track_id: i64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReconnectOutcome {
    pub relinked: Vec<Relinked>,
    pub health: LibraryHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemapOutcome {
    pub applied: RemapApplied,
    pub health: LibraryHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct UndoOutcome {
    pub undone: RemapUndone,
    pub health: LibraryHealth,
}

pub fn backfill_fingerprints(library: &SharedLibrary) -> Result<usize, String> {
    let targets = lock(library)?.fingerprint_targets().map_err(|e| format!("{e:#}"))?;
    let mut stored = 0;
    for chunk in targets.chunks(FINGERPRINT_CHUNK) {
        let fingerprints = fingerprint_files(chunk);
        stored += fingerprints.len();
        lock(library)?.store_fingerprints(&fingerprints).map_err(|e| format!("{e:#}"))?;
    }
    Ok(stored)
}

pub fn find_candidates(library: &SharedLibrary, folders: &[String]) -> Result<ReconnectPlan, String> {
    let folders: Vec<PathBuf> = folders.iter().map(PathBuf::from).collect();
    if let Some(bad) = folders.iter().find(|f| !f.is_dir()) {
        return Err(format!("{} is not a folder", bad.display()));
    }
    let inputs = lock(library)?.reconnect_inputs().map_err(|e| format!("{e:#}"))?;
    Ok(plan_reconnect(&inputs, &folders))
}

pub fn apply_picks(library: &SharedLibrary, health: &SharedHealth, picks: &[ReconnectPick]) -> Result<ReconnectOutcome, String> {
    let picks: Vec<(i64, PathBuf)> = picks.iter().map(|p| (p.track_id, PathBuf::from(&p.path))).collect();
    let relinked = lock(library)?.apply_reconnects(&picks).map_err(|e| format!("{e:#}"))?;
    let health = mark_relinked(health, relinked.iter().map(|r| r.track_id))?;
    Ok(ReconnectOutcome { relinked, health })
}

pub fn apply_remap(library: &SharedLibrary, health: &SharedHealth, old_root: &str, new_root: &str) -> Result<RemapOutcome, String> {
    let applied = lock(library)?.apply_remap(Path::new(old_root), Path::new(new_root)).map_err(|e| format!("{e:#}"))?;
    let health = mark_relinked(health, applied.remapped.iter().map(|r| r.track_id))?;
    Ok(RemapOutcome { applied, health })
}

pub fn undo_remap(library: &SharedLibrary, health: &SharedHealth, relocation_id: i64) -> Result<UndoOutcome, String> {
    let undone = lock(library)?.undo_remap(relocation_id).map_err(|e| format!("{e:#}"))?;
    let health = run_check(library, health)?;
    Ok(UndoOutcome { undone, health })
}

#[tauri::command]
pub async fn find_reconnect_candidates(folders: Vec<String>, library: State<'_, SharedLibrary>) -> Result<ReconnectPlan, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || find_candidates(&library, &folders)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_reconnects(
    picks: Vec<ReconnectPick>,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<ReconnectOutcome, String> {
    let (library, health) = (library.inner().clone(), health.inner().clone());
    tauri::async_runtime::spawn_blocking(move || apply_picks(&library, &health, &picks)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn preview_relocate_folder(old_root: String, new_root: String, library: State<'_, SharedLibrary>) -> Result<RemapPreview, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        lock(&library)?.preview_remap(Path::new(&old_root), Path::new(&new_root)).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_relocate_folder(
    old_root: String,
    new_root: String,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<RemapOutcome, String> {
    let (library, health) = (library.inner().clone(), health.inner().clone());
    tauri::async_runtime::spawn_blocking(move || apply_remap(&library, &health, &old_root, &new_root)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn undo_relocate_folder(
    relocation_id: i64,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<UndoOutcome, String> {
    let (library, health) = (library.inner().clone(), health.inner().clone());
    tauri::async_runtime::spawn_blocking(move || undo_remap(&library, &health, relocation_id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn last_folder_relocation(library: State<'_, SharedLibrary>) -> Result<Option<FolderRelocation>, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || lock(&library)?.last_folder_relocation().map_err(|e| format!("{e:#}")))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_library::LocalLibrary;
    use pioneer_usb_writer::models::Track;
    use std::sync::{Arc, Mutex};

    fn track(path: &Path, size: u64) -> Track {
        Track {
            id: 0,
            source_path: path.to_path_buf(),
            usb_path: String::new(),
            title: "t".into(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 0,
            disc_number: 0,
            track_number: 0,
            tempo: 0,
            key: String::new(),
            duration_secs: 0.0,
            sample_rate: 0,
            bitrate: 0,
            file_size: size,
            artwork: None,
        }
    }

    #[test]
    fn backfill_hashes_present_files_and_a_remap_undo_refreshes_the_health() {
        let dir = std::env::temp_dir().join(format!("ff-reconnect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("old")).unwrap();
        std::fs::create_dir_all(dir.join("new")).unwrap();
        std::fs::write(dir.join("old/a.mp3"), b"aaaa").unwrap();
        let library: SharedLibrary = Arc::new(Mutex::new(LocalLibrary::open_in_memory().unwrap()));
        let ids = lock(&library).unwrap().add_tracks(&[track(&dir.join("old/a.mp3"), 4), track(&dir.join("old/gone.mp3"), 4)]).unwrap();
        let health = SharedHealth::default();

        assert_eq!(backfill_fingerprints(&library).unwrap(), 1);
        assert_eq!(backfill_fingerprints(&library).unwrap(), 0);

        std::fs::rename(dir.join("old/a.mp3"), dir.join("new/a.mp3")).unwrap();
        let old = dir.join("old").to_string_lossy().into_owned();
        let new = dir.join("new").to_string_lossy().into_owned();
        let outcome = apply_remap(&library, &health, &old, &new).unwrap();
        assert_eq!(outcome.applied.remapped.len(), 1);
        assert_eq!(outcome.applied.still_missing, 1);
        assert!(outcome.health.relocated.contains(&ids[0]));

        let undone = undo_remap(&library, &health, outcome.applied.relocation_id).unwrap();
        assert_eq!(undone.undone.reverted.len(), 1);
        assert!(undone.health.missing.contains(&ids[0]) && undone.health.missing.contains(&ids[1]));

        let plan = find_candidates(&library, &[dir.join("new").to_string_lossy().into_owned()]).unwrap();
        assert!(plan.reconnect.is_empty() && plan.needs_choice.is_empty());
        assert_eq!(plan.still_missing, ids);
        assert!(find_candidates(&library, &[dir.join("nope").to_string_lossy().into_owned()]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
