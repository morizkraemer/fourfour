use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use pioneer_library::{FileStatus, Relinked, RelocateReport, file_status};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::analysis_run::SharedLibrary;

pub const HEALTH_EVENT: &str = "library-health";

/// Last file check, kept in memory: the files are checked again on every launch.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct LibraryHealth {
    pub running: bool,
    pub checked_at_ms: Option<u64>,
    pub duration_ms: u64,
    pub total: usize,
    pub missing: BTreeSet<i64>,
    pub error: BTreeSet<i64>,
    pub relocated: BTreeSet<i64>,
}

pub type SharedHealth = Arc<Mutex<LibraryHealth>>;

#[derive(Debug, Clone, Serialize)]
pub struct RelocateOutcome {
    pub report: RelocateReport,
    pub health: LibraryHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocateOutcome {
    pub relinked: Relinked,
    pub health: LibraryHealth,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    m.lock().map_err(|e| e.to_string())
}

/// Stats every track's file. The library lock is held only to read the paths.
pub fn run_check(library: &SharedLibrary, health: &SharedHealth) -> Result<LibraryHealth, String> {
    lock(health)?.running = true;
    let started = Instant::now();
    let files = lock(library)?.track_files().map_err(|e| e.to_string());
    let files = match files {
        Ok(files) => files,
        Err(e) => {
            lock(health)?.running = false;
            return Err(e);
        }
    };
    let (mut missing, mut error) = (BTreeSet::new(), BTreeSet::new());
    for file in &files {
        match file_status(&file.path) {
            FileStatus::Ok => {}
            FileStatus::Missing => {
                missing.insert(file.track_id);
            }
            FileStatus::Error => {
                error.insert(file.track_id);
            }
        }
    }
    let mut state = lock(health)?;
    let known: BTreeSet<i64> = files.iter().map(|f| f.track_id).collect();
    let relocated = state
        .relocated
        .iter()
        .copied()
        .filter(|id| known.contains(id) && !missing.contains(id) && !error.contains(id))
        .collect();
    *state = LibraryHealth {
        running: false,
        checked_at_ms: Some(now_ms()),
        duration_ms: started.elapsed().as_millis() as u64,
        total: files.len(),
        missing,
        error,
        relocated,
    };
    Ok(state.clone())
}

pub(crate) fn mark_relinked(health: &SharedHealth, ids: impl IntoIterator<Item = i64>) -> Result<LibraryHealth, String> {
    let mut state = lock(health)?;
    for id in ids {
        state.missing.remove(&id);
        state.error.remove(&id);
        state.relocated.insert(id);
    }
    Ok(state.clone())
}

pub fn relocate(library: &SharedLibrary, health: &SharedHealth, folder: &Path) -> Result<RelocateOutcome, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let ids: Vec<i64> = {
        let state = lock(health)?;
        state.missing.iter().chain(state.error.iter()).copied().collect()
    };
    let report = lock(library)?.relocate_in_folder(&ids, folder).map_err(|e| e.to_string())?;
    let health = mark_relinked(health, report.matched.iter().map(|r| r.track_id))?;
    Ok(RelocateOutcome { report, health })
}

pub fn locate(library: &SharedLibrary, health: &SharedHealth, track_id: i64, path: &Path) -> Result<LocateOutcome, String> {
    lock(library)?.relink_track(track_id, path).map_err(|e| format!("{e:#}"))?;
    let health = mark_relinked(health, [track_id])?;
    Ok(LocateOutcome {
        relinked: Relinked { track_id, source_path: path.to_string_lossy().into_owned() },
        health,
    })
}

/// Runs the check off the main thread and tells the window when it is done.
pub fn spawn_check(app: AppHandle, library: SharedLibrary, health: SharedHealth) {
    tauri::async_runtime::spawn_blocking(move || match run_check(&library, &health) {
        Ok(result) => {
            let _ = app.emit(HEALTH_EVENT, result);
            if let Err(e) = crate::reconnect_commands::backfill_fingerprints(&library) {
                crate::app_log::warn(&format!("content hashing failed: {e}"));
            }
        }
        Err(e) => crate::app_log::warn(&format!("library file check failed: {e}")),
    });
}

#[tauri::command]
pub async fn check_library_files(
    app: AppHandle,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<LibraryHealth, String> {
    let (library, health) = (library.inner().clone(), health.inner().clone());
    let result = tauri::async_runtime::spawn_blocking(move || run_check(&library, &health))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.emit(HEALTH_EVENT, result.clone());
    Ok(result)
}

#[tauri::command]
pub fn get_library_health(health: State<'_, SharedHealth>) -> Result<LibraryHealth, String> {
    Ok(lock(&health)?.clone())
}

#[tauri::command]
pub async fn relocate_missing_in_folder(
    folder: String,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<RelocateOutcome, String> {
    let (library, health) = (library.inner().clone(), health.inner().clone());
    tauri::async_runtime::spawn_blocking(move || relocate(&library, &health, Path::new(&folder)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command(async)]
pub fn locate_track_file(
    track_id: i64,
    path: String,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<LocateOutcome, String> {
    locate(&library, &health, track_id, Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_library::LocalLibrary;
    use pioneer_usb_writer::models::Track;
    use std::path::PathBuf;

    fn track(path: &Path, size: u64) -> Track {
        Track {
            id: 0,
            source_path: path.to_path_buf(),
            usb_path: String::new(),
            title: String::new(),
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

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("ff-health-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup(dir: &Path) -> (SharedLibrary, SharedHealth, Vec<i64>, PathBuf) {
        let music = dir.join("music");
        std::fs::create_dir_all(&music).unwrap();
        std::fs::write(music.join("here.mp3"), b"1234").unwrap();
        std::fs::create_dir_all(dir.join("moved/sub")).unwrap();
        std::fs::write(dir.join("moved/sub/gone.mp3"), b"12345").unwrap();
        let library = LocalLibrary::open_in_memory().unwrap();
        let ids = library
            .add_tracks(&[track(&music.join("here.mp3"), 4), track(&music.join("gone.mp3"), 5), track(&music.join("never.mp3"), 3)])
            .unwrap();
        (Arc::new(Mutex::new(library)), SharedHealth::default(), ids, dir.join("moved"))
    }

    #[test]
    fn check_counts_missing_tracks_and_relocate_marks_them_relocated() {
        let dir = TempDir::new("check");
        let (library, health, ids, moved) = setup(dir.path());

        let first = run_check(&library, &health).unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(first.missing, BTreeSet::from([ids[1], ids[2]]));
        assert!(first.error.is_empty() && !first.running && first.checked_at_ms.is_some());

        let outcome = relocate(&library, &health, &moved).unwrap();
        assert_eq!(outcome.report.matched.len(), 1);
        assert_eq!(outcome.report.matched[0].track_id, ids[1]);
        assert_eq!(outcome.report.not_found, vec![ids[2]]);
        assert_eq!(outcome.health.missing, BTreeSet::from([ids[2]]));
        assert_eq!(outcome.health.relocated, BTreeSet::from([ids[1]]));

        let again = run_check(&library, &health).unwrap();
        assert_eq!(again.missing, BTreeSet::from([ids[2]]));
        assert_eq!(again.relocated, BTreeSet::from([ids[1]]));
    }

    #[test]
    fn locate_relinks_one_track_and_refuses_a_missing_file() {
        let dir = TempDir::new("locate");
        let (library, health, ids, moved) = setup(dir.path());
        run_check(&library, &health).unwrap();

        assert!(locate(&library, &health, ids[2], &moved.join("nope.mp3")).is_err());
        let outcome = locate(&library, &health, ids[2], &moved.join("sub/gone.mp3")).unwrap();
        assert_eq!(outcome.relinked.track_id, ids[2]);
        assert_eq!(outcome.health.missing, BTreeSet::from([ids[1]]));
        assert_eq!(outcome.health.relocated, BTreeSet::from([ids[2]]));
    }
}
