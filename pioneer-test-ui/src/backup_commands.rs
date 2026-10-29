use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pioneer_library::LocalLibrary;
use pioneer_library::backup::{self, BackupSizes, SettingsFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::analysis_run::SharedLibrary;
use crate::file_health::{self, SharedHealth};

pub const PROGRESS_EVENT: &str = "backup-progress";
const CONFIG_SETTINGS: &str = "config.json";
const UI_SETTINGS: &str = "ui.json";
const LAST_BACKUP_KEY: &str = "last_backup";
const MACHINE_KEYS: [&str; 2] = ["library_path", LAST_BACKUP_KEY];
const SAFETY_DIR: &str = "Backups";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LastBackup {
    pub path: String,
    pub at_ms: u64,
    pub bytes: u64,
    pub includes_audio: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupStatus {
    pub sizes: BackupSizes,
    pub last: Option<LastBackup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupPreview {
    pub path: String,
    pub created_at: String,
    pub track_count: usize,
    pub playlist_count: usize,
    pub includes_audio: bool,
    pub audio_files: usize,
    pub current_tracks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreOutcome {
    pub ui_settings: Option<String>,
    pub safety_path: String,
    pub relinked: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Progress {
    phase: &'static str,
    done: u64,
    total: u64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn read_config(app: &AppHandle) -> Result<Value, String> {
    let path = crate::config_path(app)?;
    Ok(std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({})))
}

fn write_config(app: &AppHandle, config: &Value) -> Result<(), String> {
    std::fs::write(crate::config_path(app)?, config.to_string()).map_err(|e| e.to_string())
}

/// The app config as it goes into a backup: without what only fits this machine.
pub fn portable_config(config: &Value) -> Value {
    let mut out = config.clone();
    if let Some(map) = out.as_object_mut() {
        for key in MACHINE_KEYS {
            map.remove(key);
        }
    }
    out
}

/// The restored config, keeping this machine's library path and last backup.
pub fn merge_restored_config(current: &Value, restored: &Value) -> Value {
    let mut out = portable_config(restored);
    if !out.is_object() {
        out = serde_json::json!({});
    }
    for key in MACHINE_KEYS {
        if let Some(value) = current.get(key) {
            out[key] = value.clone();
        }
    }
    out
}

fn progress_emitter(app: AppHandle, phase: &'static str) -> impl FnMut(u64, u64) + Send + 'static {
    let mut last_percent = u64::MAX;
    move |done, total| {
        let percent = if total == 0 { 100 } else { done.saturating_mul(100) / total };
        if percent != last_percent {
            last_percent = percent;
            let _ = app.emit(PROGRESS_EVENT, Progress { phase, done, total });
        }
    }
}

fn lock(library: &SharedLibrary) -> Result<std::sync::MutexGuard<'_, LocalLibrary>, String> {
    library.lock().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_backup_status(app: AppHandle, library: State<'_, SharedLibrary>) -> Result<BackupStatus, String> {
    let library = library.inner().clone();
    let last = read_config(&app)?.get(LAST_BACKUP_KEY).and_then(|v| serde_json::from_value(v.clone()).ok());
    let sizes = tauri::async_runtime::spawn_blocking(move || lock(&library)?.backup_sizes().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())??;
    Ok(BackupStatus { sizes, last })
}

#[tauri::command]
pub async fn export_backup(
    path: String,
    include_audio: bool,
    ui_settings: String,
    app: AppHandle,
    library: State<'_, SharedLibrary>,
) -> Result<LastBackup, String> {
    let library = library.inner().clone();
    let mut config = read_config(&app)?;
    let settings = vec![
        SettingsFile { name: CONFIG_SETTINGS.into(), bytes: portable_config(&config).to_string().into_bytes() },
        SettingsFile { name: UI_SETTINGS.into(), bytes: ui_settings.into_bytes() },
    ];
    let mut progress = progress_emitter(app.clone(), "export");
    let dest = PathBuf::from(&path);
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<u64, String> {
        let pending = lock(&library)?.begin_backup(&dest, include_audio, &settings, &mut progress).map_err(|e| format!("{e:#}"))?;
        pending.finish(&mut progress).map_err(|e| format!("{e:#}"))?;
        Ok(std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0))
    })
    .await
    .map_err(|e| e.to_string())??;
    let last = LastBackup { path, at_ms: now_ms(), bytes, includes_audio: include_audio };
    config[LAST_BACKUP_KEY] = serde_json::to_value(&last).map_err(|e| e.to_string())?;
    write_config(&app, &config)?;
    Ok(last)
}

#[tauri::command]
pub fn read_backup(path: String, library: State<'_, SharedLibrary>) -> Result<BackupPreview, String> {
    let manifest = backup::read_backup_manifest(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    let current_tracks = lock(&library)?.get_all_tracks().map_err(|e| e.to_string())?.len();
    Ok(BackupPreview {
        path,
        created_at: manifest.created_at,
        track_count: manifest.track_count,
        playlist_count: manifest.playlist_count,
        includes_audio: manifest.includes_audio,
        audio_files: manifest.audio.len(),
        current_tracks,
    })
}

fn safety_backup_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join(SAFETY_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    Ok(dir.join(format!("before-restore-{stamp}.{}", backup::BACKUP_EXTENSION)))
}

/// Unpacks and checks the backup, saves the current library as a safety backup, then swaps
/// the library in place. Audio is never deleted; with `audio_folder` the backup's audio is
/// unpacked there and its tracks are relinked to it.
#[tauri::command]
pub async fn restore_backup(
    path: String,
    audio_folder: Option<String>,
    app: AppHandle,
    library: State<'_, SharedLibrary>,
    health: State<'_, SharedHealth>,
) -> Result<RestoreOutcome, String> {
    let shared = library.inner().clone();
    let db_path = crate::read_library_path(&app)?;
    let staging = db_path.parent().unwrap_or(Path::new(".")).join(format!(".restore-{}", now_ms()));
    let safety = safety_backup_path(&app)?;
    let mut progress = progress_emitter(app.clone(), "restore");
    let (outcome, settings) = {
        let shared = shared.clone();
        let safety = safety.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<(usize, Vec<SettingsFile>), String> {
            let audio = audio_folder.as_deref().map(Path::new);
            let unpacked = backup::unpack_backup(Path::new(&path), &staging, audio, &mut progress).map_err(|e| format!("{e:#}"))?;
            let relinked = unpacked.audio_paths.len();
            let settings = unpacked.settings.clone();
            let mut lib = lock(&shared)?;
            if let Err(e) = lib.export_backup(&safety, false, &[], &mut |_, _| {}) {
                unpacked.discard();
                return Err(format!("Could not save the current library first, nothing was changed: {e:#}"));
            }
            *lib = LocalLibrary::open_in_memory().map_err(|e| e.to_string())?;
            match backup::install_backup(unpacked, &db_path) {
                Ok(restored) => {
                    *lib = restored;
                    Ok((relinked, settings))
                }
                Err(e) => {
                    if let Ok(reopened) = LocalLibrary::open(&db_path) {
                        *lib = reopened;
                    }
                    Err(format!("Restore failed: {e:#}. Your previous library is saved at {}", safety.display()))
                }
            }
        })
        .await
        .map_err(|e| e.to_string())??
    };

    let current = read_config(&app)?;
    if let Some(restored) = settings.iter().find(|s| s.name == CONFIG_SETTINGS) {
        let restored: Value = serde_json::from_slice(&restored.bytes).unwrap_or(Value::Null);
        write_config(&app, &merge_restored_config(&current, &restored))?;
    }
    let ui_settings = settings
        .iter()
        .find(|s| s.name == UI_SETTINGS)
        .and_then(|s| String::from_utf8(s.bytes.clone()).ok());

    if let Ok(mut state) = health.lock() {
        *state = Default::default();
    }
    file_health::spawn_check(app.clone(), shared, health.inner().clone());

    Ok(RestoreOutcome { ui_settings, safety_path: safety.to_string_lossy().into_owned(), relinked: outcome })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backups_leave_out_machine_settings_and_restores_keep_them() {
        let current = json!({ "library_path": "/here/library.db", "last_backup": { "path": "/b.tar.gz" }, "analysis_backend": "rust" });
        assert_eq!(portable_config(&current), json!({ "analysis_backend": "rust" }));
        let restored = json!({ "library_path": "/elsewhere/library.db", "analysis_backend": "python", "auto_analyze_new": false });
        assert_eq!(
            merge_restored_config(&current, &restored),
            json!({ "library_path": "/here/library.db", "last_backup": { "path": "/b.tar.gz" }, "analysis_backend": "python", "auto_analyze_new": false })
        );
        assert_eq!(merge_restored_config(&json!({}), &Value::Null), json!({}));
    }
}
