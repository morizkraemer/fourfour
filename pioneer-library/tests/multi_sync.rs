mod support;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use pioneer_library::{
    LocalLibrary, NoProgress, ProgressSink, StickProgress, StickReport, StickStatus, SyncPhase,
    SyncSelection, SyncTarget, statvfs_available_bytes,
};
use pioneer_usb_writer::models::{AnalysisResult, Playlist, SyncReport, Track};
use pioneer_usb_writer::{reader, writer};
use tempfile::TempDir;

#[derive(Default)]
struct Recorder {
    progress: Mutex<Vec<StickProgress>>,
    reports: Mutex<Vec<StickReport>>,
}

impl ProgressSink for Recorder {
    fn progress(&self, progress: &StickProgress) {
        self.progress.lock().unwrap().push(progress.clone());
    }
    fn finished(&self, report: &StickReport) {
        self.reports.lock().unwrap().push(report.clone());
    }
}

struct Fixture {
    _workspace: TempDir,
    root: PathBuf,
    library: LocalLibrary,
    ids: Vec<i64>,
}

fn fixture(track_count: usize) -> Fixture {
    let workspace = TempDir::new().unwrap();
    let root = workspace.path().to_path_buf();
    let library = LocalLibrary::open(&root.join("library.db")).unwrap();
    let ids = support::populate_library(&library, &root.join("music"), track_count, 0.2);
    let half = track_count / 2;
    let warmup = library.create_playlist("Warmup").unwrap();
    let peak = library.create_playlist("Peak").unwrap();
    library.set_playlist_tracks(warmup, &ids[..half]).unwrap();
    library.set_playlist_tracks(peak, &ids[half..]).unwrap();
    Fixture { _workspace: workspace, root, library, ids }
}

fn stick(fx: &Fixture, name: &str) -> PathBuf {
    let dir = fx.root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn whole(volume: &Path) -> SyncTarget {
    SyncTarget { volume: volume.to_path_buf(), selection: SyncSelection::WholeLibrary }
}

fn files_under(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                files.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn assert_valid_export(usb: &Path, expected_tracks: usize) {
    let state = reader::read_usb_state(usb).unwrap().expect("USB state present");
    assert_eq!(state.tracks.len(), expected_tracks);
    for track in &state.tracks {
        assert!(usb.join(track.usb_path.trim_start_matches('/')).is_file());
    }
    assert!(usb.join("PIONEER/rekordbox/export.pdb").is_file());
    assert!(usb.join("PIONEER/rekordbox/exportLibrary.db").is_file());
    let anlz = files_under(&usb.join("PIONEER/USBANLZ"));
    assert_eq!(anlz.len(), expected_tracks * 2);
}

fn make_read_only(dir: &Path) -> bool {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::File::create(dir.join("probe")).is_ok();
    if writable {
        std::fs::remove_file(dir.join("probe")).unwrap();
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    !writable
}

fn plenty(_: &Path) -> Result<u64> {
    Ok(u64::MAX / 2)
}

#[test]
fn three_sticks_in_parallel_get_identical_exports() {
    let fx = fixture(24);
    let sticks: Vec<PathBuf> = ["A", "B", "C"].iter().map(|n| stick(&fx, n)).collect();
    let targets: Vec<SyncTarget> = sticks.iter().map(|s| whole(s)).collect();

    let prepared = fx.library.prepare_multi_sync(&targets).unwrap();
    let recorder = Recorder::default();
    let reports = prepared.run(&recorder);

    assert_eq!(reports.len(), 3);
    for report in &reports {
        assert_eq!(report.status, StickStatus::Done, "{:?}", report.error);
        assert_eq!(report.report.as_ref().unwrap().tracks_added, 24);
    }
    for usb in &sticks {
        assert_valid_export(usb, 24);
    }

    let reference = reader::read_usb_state(&sticks[0]).unwrap().unwrap();
    let reference_files = files_under(&sticks[0].join("Contents"));
    let reference_anlz = files_under(&sticks[0].join("PIONEER/USBANLZ"));
    assert_eq!(reference_files.len(), 24);
    for usb in &sticks[1..] {
        let state = reader::read_usb_state(usb).unwrap().unwrap();
        let titles = |s: &pioneer_usb_writer::models::ExistingUsbState| {
            let mut v: Vec<_> = s.tracks.iter().map(|t| (t.id, t.title.clone(), t.usb_path.clone())).collect();
            v.sort();
            v
        };
        assert_eq!(titles(&state), titles(&reference));
        let playlists = |s: &pioneer_usb_writer::models::ExistingUsbState| {
            let mut v: Vec<_> = s.playlists.iter().map(|p| (p.name.clone(), p.track_ids.clone())).collect();
            v.sort();
            v
        };
        assert_eq!(playlists(&state), playlists(&reference));
        assert_eq!(files_under(&usb.join("Contents")), reference_files);
        assert_eq!(files_under(&usb.join("PIONEER/USBANLZ")), reference_anlz);
    }

    let progress = recorder.progress.lock().unwrap();
    for name in ["A", "B", "C"] {
        let phases: Vec<SyncPhase> =
            progress.iter().filter(|p| p.volume == name).map(|p| p.phase).collect();
        assert_eq!(phases.first(), Some(&SyncPhase::CheckingSpace), "{name}");
        assert_eq!(phases.last(), Some(&SyncPhase::Done), "{name}");
    }
    assert_eq!(recorder.reports.lock().unwrap().len(), 3);
}

#[test]
fn second_run_is_incremental_and_removes_dropped_tracks() {
    let fx = fixture(10);
    let usb = stick(&fx, "A");
    let targets = [whole(&usb)];
    let first = fx.library.prepare_multi_sync(&targets).unwrap().run(&NoProgress);
    assert_eq!(first[0].status, StickStatus::Done);

    fx.library.remove_track(fx.ids[0]).unwrap();
    let second = fx.library.prepare_multi_sync(&targets).unwrap().run(&NoProgress);
    let report = second[0].report.as_ref().unwrap();
    assert_eq!(report.tracks_added, 0);
    assert_eq!(report.tracks_removed, 1);
    assert_valid_export(&usb, 9);
}

#[test]
fn failing_stick_leaves_the_others_complete() {
    let fx = fixture(12);
    let good_a = stick(&fx, "A");
    let locked = stick(&fx, "LOCKED");
    let good_c = stick(&fx, "C");
    if !make_read_only(&locked) {
        eprintln!("running as a user that ignores directory permissions; skipping");
        return;
    }
    let targets = [whole(&good_a), whole(&locked), whole(&good_c)];

    let recorder = Recorder::default();
    let reports = fx.library.prepare_multi_sync(&targets).unwrap().run(&recorder);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(reports[0].status, StickStatus::Done);
    assert_eq!(reports[1].status, StickStatus::Failed);
    assert!(reports[1].error.as_ref().unwrap().len() > 0);
    assert_eq!(reports[2].status, StickStatus::Done);
    assert_valid_export(&good_a, 12);
    assert_valid_export(&good_c, 12);
    assert!(!locked.join("Contents").exists());
    let progress = recorder.progress.lock().unwrap();
    assert!(progress.iter().any(|p| p.volume == "LOCKED" && p.phase == SyncPhase::Failed));
}

fn panics_on_boom(
    root: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
) -> Result<SyncReport> {
    if root.ends_with("boom") {
        panic!("injected");
    }
    writer::filesystem::sync_usb(root, tracks, analyses, playlists)
}

fn errors_on_bad(
    root: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
) -> Result<SyncReport> {
    if root.ends_with("bad") {
        anyhow::bail!("injected error");
    }
    writer::filesystem::sync_usb(root, tracks, analyses, playlists)
}

#[test]
fn panicking_and_erroring_writers_do_not_stop_other_sticks() {
    let fx = fixture(6);
    let a = stick(&fx, "A");
    let boom = stick(&fx, "boom");
    let bad = stick(&fx, "bad");
    let c = stick(&fx, "C");
    let targets = [whole(&a), whole(&boom), whole(&bad), whole(&c)];
    let prepared = fx.library.prepare_multi_sync(&targets).unwrap();

    let reports = prepared.run_with(&NoProgress, &plenty, panics_on_boom);
    assert_eq!(reports[0].status, StickStatus::Done);
    assert_eq!(reports[1].status, StickStatus::Failed);
    assert_eq!(reports[3].status, StickStatus::Done);
    assert_valid_export(&a, 6);
    assert_valid_export(&c, 6);

    let reports = prepared.run_with(&NoProgress, &plenty, errors_on_bad);
    assert_eq!(reports[2].status, StickStatus::Failed);
    assert_eq!(reports[2].error.as_deref(), Some("injected error"));
    assert_eq!(reports[3].status, StickStatus::Done);
}

#[test]
fn duplicate_and_overlapping_roots_are_rejected() {
    let fx = fixture(3);
    let a = stick(&fx, "A");
    let inner = a.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let link = fx.root.join("A-link");
    std::os::unix::fs::symlink(&a, &link).unwrap();
    let b = stick(&fx, "B");

    let err = |targets: &[SyncTarget]| {
        fx.library.prepare_multi_sync(targets).err().expect("rejected").to_string()
    };
    assert!(err(&[whole(&a), whole(&a)]).contains("selected twice"));
    assert!(err(&[whole(&a), whole(&link)]).contains("selected twice"));
    assert!(err(&[whole(&a), whole(&inner)]).contains("overlap"));
    assert!(err(&[whole(&inner), whole(&a)]).contains("overlap"));
    assert!(err(&[]).contains("No USB sticks"));
    assert!(fx.library.prepare_multi_sync(&[whole(&a), whole(&b)]).is_ok());
    assert!(files_under(&a).is_empty() && files_under(&b).is_empty());
}

#[test]
fn stick_without_room_is_refused_before_anything_is_written() {
    let fx = fixture(8);
    let roomy = stick(&fx, "ROOMY");
    let tiny = stick(&fx, "TINY");
    let targets = [whole(&roomy), whole(&tiny)];
    let probe = |path: &Path| -> Result<u64> {
        if path.ends_with("TINY") { Ok(1024) } else { Ok(u64::MAX / 2) }
    };

    let reports = fx
        .library
        .prepare_multi_sync(&targets)
        .unwrap()
        .run_with(&NoProgress, &probe, writer::filesystem::sync_usb);

    assert_eq!(reports[0].status, StickStatus::Done);
    assert_eq!(reports[1].status, StickStatus::Failed);
    let message = reports[1].error.as_ref().unwrap();
    assert!(message.starts_with("Not enough space on TINY: needs "), "{message}");
    assert!(message.ends_with("free. Nothing was written."), "{message}");
    assert!(files_under(&tiny).is_empty());
    assert_valid_export(&roomy, 8);
}

#[test]
fn real_statvfs_reports_free_space() {
    let dir = TempDir::new().unwrap();
    assert!(statvfs_available_bytes(dir.path()).unwrap() > 0);
    assert!(statvfs_available_bytes(&dir.path().join("missing")).is_err());
}

#[test]
fn playlist_selection_writes_only_chosen_tracks_and_playlists() {
    let fx = fixture(10);
    let warmup_only = stick(&fx, "WARMUP");
    let everything = stick(&fx, "ALL");
    let playlists = fx.library.get_all_playlists().unwrap();
    let warmup = playlists.iter().find(|p| p.name == "Warmup").unwrap().id as i64;
    let loose_track = fx.ids[9];

    let targets = [
        SyncTarget {
            volume: warmup_only.clone(),
            selection: SyncSelection::Playlists {
                playlist_ids: vec![warmup],
                track_ids: vec![loose_track],
            },
        },
        whole(&everything),
    ];
    let reports = fx.library.prepare_multi_sync(&targets).unwrap().run(&NoProgress);

    assert_eq!(reports[0].status, StickStatus::Done);
    assert_valid_export(&warmup_only, 6);
    let state = reader::read_usb_state(&warmup_only).unwrap().unwrap();
    assert_eq!(state.playlists.len(), 1);
    assert_eq!(state.playlists[0].name, "Warmup");
    assert_valid_export(&everything, 10);
    assert_eq!(reader::read_usb_state(&everything).unwrap().unwrap().playlists.len(), 2);
}
