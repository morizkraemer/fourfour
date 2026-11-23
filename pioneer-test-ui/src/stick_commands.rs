use std::path::Path;

use pioneer_library::sticks;
use pioneer_library::{LocalLibrary, StickContents, StickReport, SyncTarget, UsbSyncPlan, VolumeCapacity};
use serde::Serialize;
use tauri::State;

use crate::analysis_run::SharedLibrary;
use crate::command_error::{CommandError, ErrorKind};
use crate::stick_busy::BusySticks;

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
    busy: &State<'_, BusySticks>,
    edit: impl FnOnce(&LocalLibrary, &str) -> anyhow::Result<usize>,
) -> Result<(usize, StickContents), CommandError> {
    let path = Path::new(path);
    let _busy = busy.acquire(path)?;
    let lib = state.lock().map_err(|e| e.to_string())?;
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

fn stick_copy_of(root: &Path, track_id: i64, lib: &LocalLibrary) -> Result<Option<String>, CommandError> {
    let Some(track) = lib.get_track(track_id).map_err(CommandError::from_anyhow)? else {
        return Ok(None);
    };
    let relative = track.usb_path.trim_start_matches('/');
    if relative.is_empty() {
        return Ok(None);
    }
    let file = root.join(relative);
    Ok(file.is_file().then(|| file.to_string_lossy().into_owned()))
}

/// The library track's audio file on the stick at `path`, or None when the stick has no copy of it.
#[tauri::command(async)]
pub fn stick_track_path(path: String, track_id: i64, state: State<'_, SharedLibrary>) -> Result<Option<String>, CommandError> {
    let lib = state.lock().map_err(|e| e.to_string())?;
    stick_copy_of(Path::new(&path), track_id, &lib)
}

/// Adds tracks to the stick's "All tracks". Gives the stick an id on first use.
#[tauri::command(async)]
pub fn stick_add_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| lib.add_stick_tracks(id, &track_ids))
}

#[tauri::command(async)]
pub fn stick_remove_tracks(
    path: String,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| lib.remove_stick_tracks(id, &track_ids))
}

#[tauri::command(async)]
pub fn stick_add_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| lib.add_stick_playlists(id, &playlist_ids))
}

#[tauri::command(async)]
pub fn stick_remove_playlists(
    path: String,
    playlist_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| lib.remove_stick_playlists(id, &playlist_ids))
}

/// "Keep updated" for one playlist on the stick. Off gives the stick its own copy of the
/// playlist's current tracks; on drops that copy so the playlist follows the library again.
#[tauri::command(async)]
pub fn stick_set_keep_updated(
    path: String,
    playlist_id: i64,
    keep_updated: bool,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| set_keep_updated(lib, id, playlist_id, keep_updated))
}

fn set_keep_updated(lib: &LocalLibrary, stick_id: &str, playlist_id: i64, keep_updated: bool) -> anyhow::Result<usize> {
    if keep_updated {
        return lib.follow_stick_playlist(stick_id, playlist_id);
    }
    if lib.stick_contents(stick_id)?.kept_as_is.iter().any(|k| k.playlist_id == playlist_id) {
        return Ok(0);
    }
    let track_ids: Vec<i64> = lib
        .get_playlist(playlist_id)?
        .map(|p| p.track_ids.iter().map(|&id| id as i64).collect())
        .unwrap_or_default();
    lib.keep_stick_playlist(stick_id, playlist_id, &track_ids)
}

/// Sets the stick's own track list for a playlist on it (remove or reorder on the stick).
/// The playlist stops following the library; the library playlist is not changed.
#[tauri::command(async)]
pub fn stick_set_playlist_tracks(
    path: String,
    playlist_id: i64,
    track_ids: Vec<i64>,
    state: State<'_, SharedLibrary>,
    busy: State<'_, BusySticks>,
) -> Result<(usize, StickContents), CommandError> {
    edit_stick(&path, &state, &busy, |lib, id| lib.keep_stick_playlist(id, playlist_id, &track_ids))
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

    #[test]
    fn keep_updated_off_copies_the_playlist_once_and_on_follows_again() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let playlist = lib.create_playlist("Warmup").unwrap();
        lib.add_stick_playlists("s1", &[playlist]).unwrap();

        assert_eq!(set_keep_updated(&lib, "s1", playlist, false).unwrap(), 1);
        lib.keep_stick_playlist("s1", playlist, &[7, 5]).unwrap();
        assert_eq!(set_keep_updated(&lib, "s1", playlist, false).unwrap(), 0, "off twice keeps the stick's edited list");
        assert_eq!(lib.stick_contents("s1").unwrap().kept_as_is[0].track_ids, vec![7, 5]);

        assert_eq!(set_keep_updated(&lib, "s1", playlist, true).unwrap(), 1);
        assert!(lib.stick_contents("s1").unwrap().kept_as_is.is_empty());
    }

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

    fn track_at(usb_path: &str) -> pioneer_usb_writer::models::Track {
        pioneer_usb_writer::models::Track {
            id: 0,
            source_path: PathBuf::from("/music/local.mp3"),
            usb_path: usb_path.to_string(),
            title: "On stick".to_string(),
            artist: "Test".to_string(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 2026,
            disc_number: 1,
            track_number: 1,
            tempo: 12_400,
            key: "8A".to_string(),
            duration_secs: 60.0,
            sample_rate: 44_100,
            bitrate: 320,
            file_size: 1,
            artwork: None,
        }
    }

    #[test]
    fn a_track_copied_to_the_stick_plays_from_the_stick_file() {
        let stick = Stick::new("copy");
        let library = LocalLibrary::open_in_memory().unwrap();
        let id = library.add_track(&track_at("/Contents/Test/On stick.mp3")).unwrap();
        let file = stick.0.join("Contents/Test/On stick.mp3");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"audio").unwrap();
        assert_eq!(stick_copy_of(&stick.0, id, &library).unwrap(), Some(file.to_string_lossy().into_owned()));
    }

    #[test]
    fn a_track_missing_from_the_stick_or_the_library_has_no_stick_file() {
        let stick = Stick::new("missing");
        let library = LocalLibrary::open_in_memory().unwrap();
        let id = library.add_track(&track_at("/Contents/Test/Gone.mp3")).unwrap();
        assert_eq!(stick_copy_of(&stick.0, id, &library).unwrap(), None);
        assert_eq!(stick_copy_of(&stick.0, id + 999, &library).unwrap(), None);
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
