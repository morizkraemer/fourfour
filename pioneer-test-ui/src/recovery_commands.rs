use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pioneer_library::LocalLibrary;
use pioneer_library::backup::{self, SettingsFile};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::analysis_run::SharedLibrary;
use crate::backup_commands::{CONFIG_SETTINGS, UI_SETTINGS, merge_restored_config, progress_emitter, read_config, write_config};
use crate::file_health::{self, SharedHealth};

const LIBRARIES_DIR: &str = "Libraries";
const DB_FILE: &str = "library.db";
const ALREADY_OPEN: &str = "A library is already open.";

#[derive(Default, Clone)]
pub struct RecoveryGate(Arc<Mutex<()>>);

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryOutcome {
    pub library_path: String,
    pub kept_path: Option<String>,
    pub ui_settings: Option<String>,
}

struct Recovered {
    library: LocalLibrary,
    db_path: PathBuf,
    settings: Vec<SettingsFile>,
}

fn new_library_folder(libraries_dir: &Path, stamp: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(libraries_dir).map_err(|e| format!("Couldn't create a folder for the new library: {e}"))?;
    for attempt in 0..100 {
        let name = if attempt == 0 { format!("library-{stamp}") } else { format!("library-{stamp}-{}", attempt + 1) };
        let folder = libraries_dir.join(name);
        match std::fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Couldn't create a folder for the new library: {e}")),
        }
    }
    Err("Couldn't find a free folder name for the new library.".to_string())
}

fn discard_new_folder(folder: &Path) {
    let _ = std::fs::remove_dir_all(folder);
}

fn open_in(db_path: PathBuf) -> Result<Recovered, String> {
    let library = LocalLibrary::open(&db_path)
        .map_err(|e| format!("Couldn't open the library at {}: {e:#}", db_path.display()))?;
    Ok(Recovered { library, db_path, settings: Vec::new() })
}

fn retry_failed(db_path: Result<PathBuf, String>) -> Result<Recovered, String> {
    let (db_path, library) = crate::open_startup_library(db_path)?;
    Ok(Recovered { library, db_path, settings: Vec::new() })
}

fn open_existing(folder: &Path) -> Result<Recovered, String> {
    let db_path = folder.join(DB_FILE);
    if !db_path.is_file() {
        return Err("There is no fourfour library in that folder. Choose the folder that holds your library.".to_string());
    }
    open_in(db_path)
}

fn start_fresh(libraries_dir: &Path, stamp: &str) -> Result<Recovered, String> {
    let folder = new_library_folder(libraries_dir, stamp)?;
    open_in(folder.join(DB_FILE)).inspect_err(|_| discard_new_folder(&folder))
}

fn restore_into_new(
    archive: &Path,
    audio_folder: Option<&Path>,
    libraries_dir: &Path,
    stamp: &str,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Recovered, String> {
    let folder = new_library_folder(libraries_dir, stamp)?;
    let staging = folder.join(format!(".restore-{stamp}"));
    let db_path = folder.join(DB_FILE);
    let restored = backup::unpack_backup(archive, &staging, audio_folder, progress)
        .map_err(|e| format!("{e:#}"))
        .and_then(|unpacked| {
            let settings = unpacked.settings.clone();
            let library = backup::install_backup(unpacked, &db_path).map_err(|e| format!("Restore failed: {e:#}"))?;
            Ok(Recovered { library, db_path, settings })
        });
    restored.inspect_err(|_| discard_new_folder(&folder))
}

fn adopt(app: &AppHandle, recovered: Recovered) -> Result<RecoveryOutcome, String> {
    let kept_path = crate::read_library_path(app).ok().filter(|previous| *previous != recovered.db_path);
    if let Some(restored) = recovered.settings.iter().find(|s| s.name == CONFIG_SETTINGS) {
        let restored: serde_json::Value = serde_json::from_slice(&restored.bytes).unwrap_or(serde_json::Value::Null);
        write_config(app, &merge_restored_config(&read_config(app)?, &restored))?;
    }
    let ui_settings = recovered
        .settings
        .iter()
        .find(|s| s.name == UI_SETTINGS)
        .and_then(|s| String::from_utf8(s.bytes.clone()).ok());
    crate::write_library_path(app, &recovered.db_path)?;

    let library: SharedLibrary = Arc::new(Mutex::new(recovered.library));
    app.manage(library.clone());
    crate::spawn_stale_staging_cleanup(recovered.db_path.clone());
    file_health::spawn_check(app.clone(), library.clone(), app.state::<SharedHealth>().inner().clone());
    crate::spawn_cover_fill(app.clone(), library, app.state::<crate::CoverFill>().inner().clone());

    Ok(RecoveryOutcome {
        library_path: recovered.db_path.to_string_lossy().into_owned(),
        kept_path: kept_path.map(|p| p.to_string_lossy().into_owned()),
        ui_settings,
    })
}

async fn recover(app: AppHandle, open: impl FnOnce(&AppHandle) -> Result<Recovered, String> + Send + 'static) -> Result<RecoveryOutcome, String> {
    let gate = app.state::<RecoveryGate>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _turn = gate.0.lock().map_err(|e| e.to_string())?;
        if app.try_state::<SharedLibrary>().is_some() {
            return Err(ALREADY_OPEN.to_string());
        }
        let recovered = open(&app)?;
        adopt(&app, recovered)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn libraries_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join(LIBRARIES_DIR))
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string()
}

#[tauri::command]
pub async fn recover_retry_library(app: AppHandle) -> Result<RecoveryOutcome, String> {
    recover(app, |app| retry_failed(crate::read_library_path(app))).await
}

#[tauri::command]
pub async fn recover_open_library(folder_path: String, app: AppHandle) -> Result<RecoveryOutcome, String> {
    recover(app, move |_| open_existing(Path::new(&folder_path))).await
}

#[tauri::command]
pub async fn recover_new_library(app: AppHandle) -> Result<RecoveryOutcome, String> {
    recover(app, |app| start_fresh(&libraries_dir(app)?, &stamp())).await
}

#[tauri::command]
pub async fn recover_restore_backup(path: String, audio_folder: Option<String>, app: AppHandle) -> Result<RecoveryOutcome, String> {
    let mut progress = progress_emitter(app.clone(), "restore");
    recover(app, move |app| {
        let audio = audio_folder.as_deref().map(Path::new);
        restore_into_new(Path::new(&path), audio, &libraries_dir(app)?, &stamp(), &mut progress)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::models::Track;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("ff-recover-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Failed {
        folder: PathBuf,
        db: PathBuf,
        bytes: Vec<u8>,
    }

    impl Failed {
        fn damaged_in(root: &Path) -> Self {
            let folder = root.join("app-data");
            std::fs::create_dir_all(&folder).unwrap();
            let db = folder.join(DB_FILE);
            let bytes = b"SQLite format 3\0 but the rest is junk".repeat(200);
            std::fs::write(&db, &bytes).unwrap();
            Self { folder, db, bytes }
        }

        fn assert_untouched(&self) {
            assert_eq!(std::fs::read(&self.db).unwrap(), self.bytes);
            let mut names: Vec<_> = std::fs::read_dir(&self.folder).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
            names.sort();
            assert!(names.iter().all(|n| n == DB_FILE || n == LIBRARIES_DIR), "side files next to the failed library: {names:?}");
        }

        fn libraries(&self) -> PathBuf {
            self.folder.join(LIBRARIES_DIR)
        }
    }

    fn track(path: &Path) -> Track {
        Track {
            id: 0,
            source_path: path.to_path_buf(),
            usb_path: String::new(),
            title: "Kept".into(),
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
            file_size: 4,
            artwork: None,
        }
    }

    fn make_backup(root: &Path) -> PathBuf {
        let folder = root.join("source");
        std::fs::create_dir_all(&folder).unwrap();
        let library = LocalLibrary::open(&folder.join(DB_FILE)).unwrap();
        library.add_track(&track(&root.join("kept.mp3"))).unwrap();
        let archive = root.join("backup.tar.gz");
        let settings = vec![SettingsFile { name: UI_SETTINGS.into(), bytes: br#"{"fourfour.quantize.v1":"1"}"#.to_vec() }];
        library.export_backup(&archive, false, &settings, &mut |_, _| {}).unwrap();
        archive
    }

    fn healthy_library(root: &Path) -> PathBuf {
        let folder = root.join("other");
        std::fs::create_dir_all(&folder).unwrap();
        let library = LocalLibrary::open(&folder.join(DB_FILE)).unwrap();
        library.add_track(&track(&root.join("kept.mp3"))).unwrap();
        folder
    }

    #[test]
    fn opening_another_library_works_without_a_library_state_and_leaves_the_failed_file_alone() {
        let dir = Scratch::new("open");
        let failed = Failed::damaged_in(dir.path());
        let recovered = open_existing(&healthy_library(dir.path())).unwrap();
        assert_eq!(recovered.library.get_all_tracks().unwrap().len(), 1);
        assert_ne!(recovered.db_path, failed.db);
        failed.assert_untouched();
    }

    #[test]
    fn opening_a_folder_without_a_library_creates_nothing() {
        let dir = Scratch::new("open-empty");
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let message = open_existing(&empty).map(|_| ()).unwrap_err();
        assert!(message.contains("no fourfour library"), "{message}");
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
    }

    #[test]
    fn opening_the_failed_folder_again_is_an_error_and_changes_nothing() {
        let dir = Scratch::new("open-same");
        let failed = Failed::damaged_in(dir.path());
        let message = open_existing(&failed.folder).map(|_| ()).unwrap_err();
        assert!(message.starts_with("Couldn't open the library"), "{message}");
        failed.assert_untouched();
    }

    #[test]
    fn retrying_a_failed_library_is_an_error_and_changes_nothing() {
        let dir = Scratch::new("retry");
        let failed = Failed::damaged_in(dir.path());
        let message = retry_failed(Ok(failed.db.clone())).map(|_| ()).unwrap_err();
        assert!(message.starts_with("Couldn't open the library"), "{message}");
        failed.assert_untouched();
    }

    #[test]
    fn retrying_opens_a_library_that_became_readable() {
        let dir = Scratch::new("retry-ok");
        let folder = healthy_library(dir.path());
        let recovered = retry_failed(Ok(folder.join(DB_FILE))).unwrap();
        assert_eq!(recovered.library.get_all_tracks().unwrap().len(), 1);
    }

    #[test]
    fn a_new_library_is_a_fresh_one_in_a_new_folder_and_the_failed_file_stays() {
        let dir = Scratch::new("new");
        let failed = Failed::damaged_in(dir.path());
        let first = start_fresh(&failed.libraries(), "2026-11-02-100000").unwrap();
        let second = start_fresh(&failed.libraries(), "2026-11-02-100000").unwrap();
        assert!(first.library.get_all_tracks().unwrap().is_empty());
        assert_eq!(first.db_path, failed.libraries().join("library-2026-11-02-100000").join(DB_FILE));
        assert_ne!(first.db_path, second.db_path, "a second new library never reuses the folder");
        failed.assert_untouched();
    }

    #[test]
    fn restoring_a_backup_builds_a_new_library_and_the_failed_file_stays() {
        let dir = Scratch::new("restore");
        let failed = Failed::damaged_in(dir.path());
        let archive = make_backup(dir.path());
        let recovered = restore_into_new(&archive, None, &failed.libraries(), "2026-11-02-100000", &mut |_, _| {}).unwrap();
        assert_eq!(recovered.library.get_all_tracks().unwrap().len(), 1);
        assert_eq!(recovered.db_path.parent().unwrap().parent().unwrap(), failed.libraries());
        assert!(recovered.settings.iter().any(|s| s.name == UI_SETTINGS));
        let leftovers: Vec<_> = std::fs::read_dir(recovered.db_path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with(".restore"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        failed.assert_untouched();
    }

    #[test]
    fn restoring_a_file_that_is_not_a_backup_fails_and_leaves_nothing_behind() {
        let dir = Scratch::new("restore-bad");
        let failed = Failed::damaged_in(dir.path());
        let not_a_backup = dir.path().join("notes.tar.gz");
        std::fs::write(&not_a_backup, b"not a backup").unwrap();
        let message = restore_into_new(&not_a_backup, None, &failed.libraries(), "2026-11-02-100000", &mut |_, _| {}).map(|_| ()).unwrap_err();
        assert!(!message.is_empty());
        assert_eq!(std::fs::read_dir(failed.libraries()).unwrap().count(), 0, "the half-made folder is removed");
        failed.assert_untouched();
    }
}
