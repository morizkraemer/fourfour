use std::path::Path;

use pioneer_library::sticks;
use pioneer_library::{LocalLibrary, StickContents, StickReport, SyncTarget, UsbSyncPlan, VolumeCapacity};
use serde::Serialize;
use tauri::State;

use crate::analysis_run::SharedLibrary;
use crate::command_error::{CommandError, ErrorKind};

/// A stick's report with the kind of its failure. The sync engine reports failures as text only, so the
/// kind is read from the engine's own sentences here; the tests below run the real engine against them.
#[derive(Clone, Serialize)]
pub struct StickReportWire {
    #[serde(flatten)]
    report: StickReport,
    error_kind: Option<ErrorKind>,
}

impl From<StickReport> for StickReportWire {
    fn from(report: StickReport) -> Self {
        let error_kind = report.error.as_deref().map(sync_error_kind);
        Self { report, error_kind }
    }
}

pub fn sync_error_kind(message: &str) -> ErrorKind {
    if message.starts_with("Not enough space on ") {
        ErrorKind::NoSpaceOnStick
    } else if message.starts_with("Cannot open ") || message.starts_with("Cannot read free space on ") {
        ErrorKind::DriveGone
    } else if message.ends_with(" is already being synced.") {
        ErrorKind::StickBusy
    } else {
        ErrorKind::Other
    }
}

fn volume_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn known_stick(path: &Path, lib: &LocalLibrary) -> Result<StickContents, CommandError> {
    match sticks::read_stick_id(path).map_err(|e| CommandError::from_anyhow_chain(e).with_path(path).for_stick())? {
        Some(id) => lib.stick_contents(&id).map_err(CommandError::from_anyhow),
        None => Ok(StickContents::default()),
    }
}

fn claimed_stick(path: &Path, lib: &LocalLibrary) -> Result<String, CommandError> {
    if !path.is_dir() {
        return Err(CommandError::new(ErrorKind::DriveGone, format!("{} is not mounted.", path.display())).with_path(path));
    }
    crate::volumes::require_removable_volume(path)?;
    let id = sticks::ensure_stick_id(path).map_err(|e| CommandError::from_anyhow_chain(e).with_path(path).for_stick())?;
    lib.remember_stick(&id, &volume_name(path), path).map_err(CommandError::from_anyhow)?;
    Ok(id)
}

fn edit_stick(
    path: &str,
    state: &State<'_, SharedLibrary>,
    edit: impl FnOnce(&LocalLibrary, &str) -> anyhow::Result<usize>,
) -> Result<(usize, StickContents), CommandError> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    let path = Path::new(path);
    let id = claimed_stick(path, &lib)?;
    let changed = edit(&lib, &id).map_err(CommandError::from_anyhow)?;
    Ok((changed, lib.stick_contents(&id).map_err(CommandError::from_anyhow)?))
}

/// What the library remembers for the stick at `path`; empty for a stick fourfour never touched.
#[tauri::command(async)]
pub fn get_stick(path: String, state: State<'_, SharedLibrary>) -> Result<StickContents, CommandError> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    known_stick(Path::new(&path), &lib)
}

/// Adds tracks to the stick's "All tracks". Gives the stick an id on first use.
#[tauri::command(async)]
pub fn stick_add_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, |lib, id| lib.add_stick_tracks(id, &track_ids))
}

#[tauri::command(async)]
pub fn stick_remove_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, |lib, id| lib.remove_stick_tracks(id, &track_ids))
}

#[tauri::command(async)]
pub fn stick_add_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, |lib, id| lib.add_stick_playlists(id, &playlist_ids))
}

#[tauri::command(async)]
pub fn stick_remove_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, |lib, id| lib.remove_stick_playlists(id, &playlist_ids))
}

#[tauri::command(async)]
pub fn get_volume_capacity(path: String) -> Result<VolumeCapacity, CommandError> {
    pioneer_library::volume_capacity(Path::new(&path))
        .map_err(|e| CommandError::from_anyhow_chain(e).with_path(&path).for_stick())
}

/// Dry run of a sync: what `sync_usb_many` with the same target would do. Writes nothing.
#[tauri::command]
pub async fn plan_usb_sync(
    target: SyncTarget,
    state: State<'_, SharedLibrary>,
) -> Result<UsbSyncPlan, CommandError> {
    let shared: SharedLibrary = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = crate::import_plan::prepare_sync_with_covers(&shared, &[target])?;
        prepared
            .plan()
            .into_iter()
            .next()
            .ok_or_else(|| CommandError::other("No USB stick selected."))?
            .map_err(|e| {
                let message = format!("{e:#}");
                let kind = CommandError::from_anyhow(e).for_stick().kind;
                CommandError::new(if kind == ErrorKind::Other { sync_error_kind(&message) } else { kind }, message)
            })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use pioneer_library::{LocalLibrary, NoProgress, StickStatus, SyncSelection, SyncTarget};
    use pioneer_usb_writer::writer::filesystem::sync_usb_keeping;

    use super::*;

    struct Stick(PathBuf);

    impl Stick {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("ff-stick-kinds-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Stick {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn whole(volume: &Path) -> SyncTarget {
        SyncTarget { volume: volume.to_path_buf(), selection: SyncSelection::WholeLibrary }
    }

    fn only_report(library: &LocalLibrary, target: SyncTarget, free: fn(&Path) -> anyhow::Result<u64>) -> StickReport {
        let prepared = library.prepare_multi_sync(&[target]).unwrap();
        let mut reports = prepared.run_with(&NoProgress, &free, sync_usb_keeping);
        assert_eq!(reports.len(), 1);
        reports.remove(0)
    }

    #[test]
    fn a_stick_without_room_is_no_space_on_stick_in_the_engines_own_words() {
        let stick = Stick::new("room");
        let library = LocalLibrary::open_in_memory().unwrap();
        let report = only_report(&library, whole(&stick.0), |_| Ok(0));
        assert_eq!(report.status, StickStatus::Failed);
        let wire = StickReportWire::from(report);
        assert!(wire.report.error.as_deref().unwrap().starts_with("Not enough space on "), "{:?}", wire.report.error);
        assert_eq!(wire.error_kind, Some(ErrorKind::NoSpaceOnStick));
    }

    #[test]
    fn a_stick_that_cannot_be_opened_is_drive_gone_in_the_engines_own_words() {
        let library = LocalLibrary::open_in_memory().unwrap();
        let report = only_report(&library, whole(Path::new("/definitely/not/a/stick")), |_| Ok(u64::MAX / 2));
        let wire = StickReportWire::from(report);
        assert!(wire.report.error.as_deref().unwrap().starts_with("Cannot open "), "{:?}", wire.report.error);
        assert_eq!(wire.error_kind, Some(ErrorKind::DriveGone));
    }

    #[test]
    fn a_free_space_probe_that_fails_is_drive_gone_in_the_engines_own_words() {
        let stick = Stick::new("probe");
        let library = LocalLibrary::open_in_memory().unwrap();
        let report = only_report(&library, whole(&stick.0), |_| Err(anyhow::anyhow!("statvfs failed")));
        let wire = StickReportWire::from(report);
        assert!(wire.report.error.as_deref().unwrap().starts_with("Cannot read free space on "), "{:?}", wire.report.error);
        assert_eq!(wire.error_kind, Some(ErrorKind::DriveGone));
    }

    #[test]
    fn a_stick_that_is_already_syncing_is_stick_busy_in_the_engines_own_words() {
        let stick = Stick::new("busy");
        let library = LocalLibrary::open_in_memory().unwrap();
        let first = library.prepare_multi_sync(&[whole(&stick.0)]).unwrap();
        let second = library.prepare_multi_sync(&[whole(&stick.0)]).unwrap();
        static HOLDING: AtomicBool = AtomicBool::new(false);
        static RELEASE: AtomicBool = AtomicBool::new(false);
        fn holding(_: &Path) -> anyhow::Result<u64> {
            HOLDING.store(true, Ordering::SeqCst);
            while !RELEASE.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(0)
        }
        let report = std::thread::scope(|scope| {
            let running = scope.spawn(|| first.run_with(&NoProgress, &holding, sync_usb_keeping));
            while !HOLDING.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut reports = second.run_with(&NoProgress, &|_| Ok(u64::MAX / 2), sync_usb_keeping);
            RELEASE.store(true, Ordering::SeqCst);
            running.join().unwrap();
            reports.remove(0)
        });
        let wire = StickReportWire::from(report);
        assert!(wire.report.error.as_deref().unwrap().ends_with(" is already being synced."), "{:?}", wire.report.error);
        assert_eq!(wire.error_kind, Some(ErrorKind::StickBusy));
    }

    #[test]
    fn a_report_without_an_error_has_no_kind_and_serialises_flat() {
        let stick = Stick::new("fine");
        let library = LocalLibrary::open_in_memory().unwrap();
        let report = only_report(&library, whole(&stick.0), |_| Ok(u64::MAX / 2));
        assert_eq!(report.status, StickStatus::Done, "{:?}", report.error);
        let json = serde_json::to_value(StickReportWire::from(report)).unwrap();
        assert_eq!(json["error"], serde_json::Value::Null);
        assert_eq!(json["error_kind"], serde_json::Value::Null);
        assert_eq!(json["status"], "done");
    }

    #[test]
    fn a_volume_that_cannot_be_resolved_is_drive_gone_and_a_system_folder_is_refused_as_other() {
        let gone = crate::volumes::require_removable_volume(Path::new("/definitely/not/a/stick")).unwrap_err();
        assert_eq!(gone.kind, ErrorKind::DriveGone);
        assert!(gone.message.starts_with("Cannot resolve /definitely/not/a/stick: "), "{gone}");
        assert_eq!(gone.path.as_deref(), Some("/definitely/not/a/stick"));
        let refused = crate::volumes::require_removable_volume(&std::env::temp_dir()).unwrap_err();
        assert_eq!(refused.kind, ErrorKind::Other);
    }

    #[test]
    fn other_messages_are_other() {
        assert_eq!(sync_error_kind("Sync thread panicked."), ErrorKind::Other);
        assert_eq!(sync_error_kind("The writer failed"), ErrorKind::Other);
    }

    #[test]
    fn a_stick_that_is_not_mounted_is_drive_gone_with_its_path() {
        let library = LocalLibrary::open_in_memory().unwrap();
        let error = claimed_stick(Path::new("/definitely/not/a/stick"), &library).unwrap_err();
        assert_eq!(error.kind, ErrorKind::DriveGone);
        assert_eq!(error.message, "/definitely/not/a/stick is not mounted.");
        assert_eq!(error.path.as_deref(), Some("/definitely/not/a/stick"));
    }
}
