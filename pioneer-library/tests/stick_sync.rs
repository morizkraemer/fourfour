mod support;

use std::path::{Path, PathBuf};

use anyhow::Result;
use pioneer_library::rekordbox::ImportedCue;
use pioneer_library::sticks::{self, MANIFEST_REL_PATH};
use pioneer_library::{
    LocalLibrary, NoProgress, StickStatus, SyncSelection, SyncTarget, UsbSyncPlan, volume_capacity,
};
use pioneer_usb_writer::models::ExistingUsbState;
use pioneer_usb_writer::reader;
use pioneer_usb_writer::writer;
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    root: PathBuf,
    library: LocalLibrary,
    ids: Vec<i64>,
    warmup: i64,
    peak: i64,
}

fn fixture(track_count: usize) -> Fixture {
    let workspace = TempDir::new().unwrap();
    let root = workspace.path().to_path_buf();
    let library = LocalLibrary::open(&root.join("library.db")).unwrap();
    let ids = support::populate_library(&library, &root.join("music"), track_count, 0.2);
    let warmup = library.create_playlist("Warmup").unwrap();
    let peak = library.create_playlist("Peak").unwrap();
    library.set_playlist_tracks(warmup, &ids[..2]).unwrap();
    library.set_playlist_tracks(peak, &ids[2..4]).unwrap();
    Fixture { _workspace: workspace, root, library, ids, warmup, peak }
}

fn fake_stick(fx: &Fixture, name: &str) -> PathBuf {
    let dir = fx.root.join("fake-volumes").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn target(volume: &Path, playlist_ids: &[i64], track_ids: &[i64]) -> SyncTarget {
    SyncTarget {
        volume: volume.to_path_buf(),
        selection: SyncSelection::Playlists {
            playlist_ids: playlist_ids.to_vec(),
            track_ids: track_ids.to_vec(),
        },
    }
}

fn plan(fx: &Fixture, target: SyncTarget, free: u64) -> UsbSyncPlan {
    let probe = move |_: &Path| -> Result<u64> { Ok(free) };
    fx.library.prepare_multi_sync(&[target]).unwrap().plan_with(&probe).remove(0).unwrap()
}

fn sync(fx: &Fixture, target: SyncTarget) {
    let reports = fx.library.prepare_multi_sync(&[target]).unwrap().run(&NoProgress);
    assert_eq!(reports[0].status, StickStatus::Done, "{:?}", reports[0].error);
}

fn state(usb: &Path) -> ExistingUsbState {
    reader::read_usb_state(usb).unwrap().expect("stick has a library")
}

fn titles(state: &ExistingUsbState) -> Vec<String> {
    let mut v: Vec<String> = state.tracks.iter().map(|t| t.title.clone()).collect();
    v.sort();
    v
}

fn library_title(fx: &Fixture, id: i64) -> String {
    fx.library.get_track(id).unwrap().unwrap().title
}

const PLENTY: u64 = u64::MAX / 2;

#[test]
fn selection_plan_lists_adds_and_unanalyzed_and_writes_nothing() {
    let fx = fixture(8);
    let raw = fx.root.join("music-raw");
    support::write_wav(&raw.join("raw/unanalyzed.wav"), 220.0, 0.2);
    let raw_ids = fx
        .library
        .add_tracks(&pioneer_usb_writer::scanner::scan_directory(&raw).unwrap())
        .unwrap();
    let usb = fake_stick(&fx, "STICK");

    let plan = plan(&fx, target(&usb, &[fx.warmup], &[fx.ids[6], raw_ids[0]]), PLENTY);

    assert_eq!(plan.playlist_count, 1);
    let mut added: Vec<i64> = plan.add.iter().map(|t| t.track_id.unwrap()).collect();
    added.sort();
    assert_eq!(added, vec![fx.ids[0], fx.ids[1], fx.ids[6]]);
    assert!(plan.remove.is_empty());
    assert_eq!(plan.skipped_unanalyzed.len(), 1);
    assert_eq!(plan.skipped_unanalyzed[0].track_id, Some(raw_ids[0]));
    assert_eq!(plan.skipped_unanalyzed[0].title, library_title(&fx, raw_ids[0]));
    assert_eq!(plan.bytes_to_add, plan.add.iter().map(|t| t.size_bytes.unwrap()).sum::<u64>());
    assert!(plan.bytes_needed > plan.bytes_to_add);
    assert_eq!(plan.fits, Some(true));
    assert_eq!(plan.stick_id, None);
    assert_eq!(std::fs::read_dir(&usb).unwrap().count(), 0, "a plan writes nothing");
}

#[test]
fn plan_says_wont_fit_and_the_write_is_refused() {
    let fx = fixture(4);
    let usb = fake_stick(&fx, "TINY");
    let tiny = plan(&fx, target(&usb, &[fx.warmup, fx.peak], &[]), 1024);
    assert_eq!(tiny.fits, Some(false));
    assert_eq!(tiny.free_bytes, Some(1024));

    let probe = |_: &Path| -> Result<u64> { Ok(1024) };
    let reports = fx
        .library
        .prepare_multi_sync(&[target(&usb, &[fx.warmup], &[])])
        .unwrap()
        .run_with(&NoProgress, &probe, writer::filesystem::sync_usb_keeping);
    assert_eq!(reports[0].status, StickStatus::Failed);
    assert!(reports[0].error.as_ref().unwrap().contains("Not enough space"));
    assert_eq!(std::fs::read_dir(&usb).unwrap().count(), 0);
}

#[test]
fn capacity_of_a_fake_volume() {
    let dir = TempDir::new().unwrap();
    let capacity = volume_capacity(dir.path()).unwrap();
    assert!(capacity.total_bytes > 0);
    assert!(capacity.free_bytes <= capacity.total_bytes);
    assert!(volume_capacity(&dir.path().join("missing")).is_err());
}

#[test]
fn stick_id_survives_remount_and_rename_and_keys_the_library_lists() {
    let fx = fixture(4);
    let usb = fake_stick(&fx, "SANDISK");
    let id = sticks::ensure_stick_id(&usb).unwrap();
    fx.library.remember_stick(&id, "SANDISK", &usb).unwrap();
    fx.library.add_stick_tracks(&id, &[fx.ids[3]]).unwrap();
    fx.library.add_stick_playlists(&id, &[fx.warmup]).unwrap();
    sync(&fx, target(&usb, &[fx.warmup], &[fx.ids[3]]));
    assert_eq!(sticks::read_stick_id(&usb).unwrap().as_deref(), Some(id.as_str()));

    let remounted = fx.root.join("other-mount").join("RENAMED");
    std::fs::create_dir_all(remounted.parent().unwrap()).unwrap();
    std::fs::rename(&usb, &remounted).unwrap();
    let found = sticks::read_stick_id(&remounted).unwrap().unwrap();
    assert_eq!(found, id);
    let contents = fx.library.stick_contents(&found).unwrap();
    assert_eq!(contents.track_ids, vec![fx.ids[3]]);
    assert_eq!(contents.playlist_ids, vec![fx.warmup]);
    assert_eq!(plan(&fx, target(&remounted, &[fx.warmup], &[fx.ids[3]]), PLENTY).add.len(), 0);
}

#[test]
fn removals_only_touch_files_fourfour_wrote() {
    let fx = fixture(6);
    let usb = fake_stick(&fx, "SHARED");

    let (tracks, analyses) = fx.library.get_analyzed_tracks().unwrap();
    let foreign: Vec<_> = tracks.iter().zip(&analyses).filter(|(t, _)| t.id as i64 == fx.ids[5]).collect();
    let mut foreign_track = foreign[0].0.clone();
    foreign_track.id = 1;
    foreign_track.usb_path = "/Contents/Elsewhere/foreign.wav".into();
    writer::filesystem::sync_usb(&usb, &[foreign_track], &[foreign[0].1.clone()], &[]).unwrap();
    let loose = usb.join("Contents/notes.txt");
    std::fs::write(&loose, b"mine").unwrap();
    assert!(!usb.join(MANIFEST_REL_PATH).exists());

    let first = plan(&fx, target(&usb, &[fx.warmup], &[]), PLENTY);
    assert!(first.remove.is_empty());
    assert_eq!(first.foreign_kept_count, 1);
    sync(&fx, target(&usb, &[fx.warmup], &[]));
    assert!(usb.join("Contents/Elsewhere/foreign.wav").is_file(), "foreign audio stays");
    assert!(loose.is_file());
    assert_eq!(state(&usb).tracks.len(), 2);

    let second = plan(&fx, target(&usb, &[], &[fx.ids[0]]), PLENTY);
    assert_eq!(second.remove.len(), 1);
    assert_eq!(second.remove[0].track_id, Some(fx.ids[1]));
    sync(&fx, target(&usb, &[], &[fx.ids[0]]));
    assert!(usb.join("Contents/Elsewhere/foreign.wav").is_file());
    assert!(loose.is_file());
    assert_eq!(titles(&state(&usb)), vec![library_title(&fx, fx.ids[0])]);
}

#[test]
fn resync_after_removing_a_track_drops_only_that_track() {
    let fx = fixture(6);
    let usb = fake_stick(&fx, "STICK");
    let cue = |n: u32, ms: u32| ImportedCue { hot_cue_number: n, time_ms: ms, loop_time_ms: None, color: None, name: String::new() };
    fx.library.set_cues(fx.ids[0], &[cue(1, 50), cue(2, 120)]).unwrap();

    sync(&fx, target(&usb, &[fx.warmup, fx.peak], &[fx.ids[4], fx.ids[5]]));
    let before = state(&usb);
    assert_eq!(before.tracks.len(), 6);
    let mut names: Vec<_> = before.playlists.iter().map(|p| p.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["Peak", "Warmup"]);
    let warmup_first = &before.tracks.iter().find(|t| t.title == library_title(&fx, fx.ids[0])).unwrap();
    let dat = usb
        .join(writer::anlz::anlz_dir_for_path(&warmup_first.usb_path).trim_start_matches('/'))
        .join("ANLZ0000.DAT");
    let analysis = reader::anlz::read_anlz(&dat).unwrap();
    assert_eq!(analysis.cue_points.iter().filter(|c| c.hot_cue_number > 0).count(), 2);
    let manifest = sticks::read_manifest(&usb).unwrap().unwrap();
    assert_eq!(manifest.files.len(), 6);
    assert!(manifest.last_sync.is_some());

    let dropped = before.tracks.iter().find(|t| t.title == library_title(&fx, fx.ids[5])).unwrap().clone();
    let next = target(&usb, &[fx.warmup, fx.peak], &[fx.ids[4]]);
    let planned = plan(&fx, next.clone(), PLENTY);
    assert_eq!(planned.remove.len(), 1);
    assert_eq!(planned.add.len(), 0);
    assert_eq!(planned.unchanged_count + planned.update_count, 5, "{planned:?}");
    sync(&fx, next);

    let after = state(&usb);
    assert_eq!(after.tracks.len(), 5);
    assert!(!usb.join(dropped.usb_path.trim_start_matches('/')).exists());
    for track in &after.tracks {
        assert!(usb.join(track.usb_path.trim_start_matches('/')).is_file());
    }
    assert_eq!(after.playlists.len(), 2);
    assert_eq!(after.playlists.iter().map(|p| p.track_ids.len()).sum::<usize>(), 4);
    assert!(reader::anlz::read_anlz(&dat).is_ok());
    assert_eq!(sticks::read_manifest(&usb).unwrap().unwrap().files.len(), 5);

    let pdb = std::fs::read(usb.join("PIONEER/rekordbox/export.pdb")).unwrap();
    assert!(!pdb.is_empty() && pdb.len() % 4096 == 0);
    let imported = pioneer_library::rekordbox::load_usb(&usb).unwrap();
    assert_eq!(imported.tracks.len(), 5);
    let mut imported_names: Vec<_> =
        imported.playlists.iter().filter(|p| !p.is_folder).map(|p| p.name.clone()).collect();
    imported_names.sort();
    assert_eq!(imported_names, vec!["Peak", "Warmup"]);
    let with_cues = imported
        .tracks
        .iter()
        .find(|t| t.track.title == library_title(&fx, fx.ids[0]))
        .unwrap();
    assert_eq!(with_cues.cues.iter().filter(|c| c.hot_cue_number > 0).count(), 2);
}

fn playlist(id: i64, name: &str, tracks: &[i64]) -> pioneer_usb_writer::models::Playlist {
    pioneer_usb_writer::models::Playlist {
        id: id as u32,
        name: name.to_string(),
        track_ids: tracks.iter().map(|&t| t as u32).collect(),
    }
}

#[test]
fn saving_playlists_keeps_their_ids_and_the_sticks_holding_them() {
    let fx = fixture(6);
    let stick = "stick-a";
    fx.library.add_stick_playlists(stick, &[fx.peak]).unwrap();

    fx.library
        .replace_playlists(&[playlist(fx.warmup, "Warmup", &fx.ids[..1]), playlist(fx.peak, "Peak", &fx.ids[2..4])])
        .unwrap();
    assert_eq!(fx.library.stick_contents(stick).unwrap().playlist_ids, vec![fx.peak]);

    let added = fx.peak + 7;
    fx.library
        .replace_playlists(&[playlist(fx.peak, "Peak", &fx.ids[2..5]), playlist(added, "New", &fx.ids[..1])])
        .unwrap();
    let summary: Vec<(i64, String, usize)> = fx
        .library
        .get_all_playlists()
        .unwrap()
        .iter()
        .map(|p| (p.id as i64, p.name.clone(), p.track_ids.len()))
        .collect();
    assert_eq!(summary, vec![(fx.peak, "Peak".into(), 3), (added, "New".into(), 1)]);
    assert_eq!(fx.library.stick_contents(stick).unwrap().playlist_ids, vec![fx.peak]);
}
