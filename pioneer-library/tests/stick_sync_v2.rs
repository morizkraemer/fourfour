mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pioneer_library::sticks::{self, MANIFEST_REL_PATH};
use pioneer_library::{LocalLibrary, NoProgress, StickStatus, SyncSelection, SyncTarget};
use pioneer_usb_writer::models::SyncReport;
use pioneer_usb_writer::stick::{one_library, Stick, StickModel};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    root: PathBuf,
    library: LocalLibrary,
    ids: Vec<i64>,
}

fn fixture(name: &str, track_count: usize) -> Fixture {
    let workspace = TempDir::new().unwrap();
    let root = workspace.path().to_path_buf();
    let library = LocalLibrary::open(&root.join("library.db")).unwrap();
    let ids = support::populate_library(&library, &root.join(name), track_count, 0.2);
    let warmup = library.create_playlist(&format!("{name} warmup")).unwrap();
    library.set_playlist_tracks(warmup, &ids[..2]).unwrap();
    Fixture { _workspace: workspace, root, library, ids }
}

fn stick_dir(fx: &Fixture) -> PathBuf {
    let dir = fx.root.join("volumes").join("STICK");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn counts(report: &SyncReport) -> (u32, u32, u32, u32, u32) {
    (report.tracks_added, report.tracks_updated, report.tracks_replaced, report.tracks_removed, report.tracks_unchanged)
}

fn listing(root: &Path) -> BTreeMap<String, (u64, SystemTime)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                let rel = entry.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(rel, (meta.len(), meta.modified().unwrap()));
            }
        }
    }
    out
}

#[test]
fn v2_sync_records_ownership_and_a_second_sync_touches_nothing() {
    let fx = fixture("music", 4);
    let usb = stick_dir(&fx);
    assert_eq!(counts(&fx.library.sync_usb_v2(&usb, "STICK").unwrap()), (4, 0, 0, 0, 0));

    let manifest = sticks::read_manifest(&usb).unwrap().unwrap();
    let owned = manifest.v2.as_ref().unwrap();
    assert_eq!((manifest.files.len(), owned.paths.len(), owned.playlist_ids.len(), owned.fingerprints.len()), (4, 4, 1, 4));
    let mut recorded: Vec<i64> = manifest.files.iter().map(|file| file.track_id).collect();
    recorded.sort();
    assert_eq!(recorded, fx.ids);

    let before = listing(&usb);
    assert_eq!(counts(&fx.library.sync_usb_v2(&usb, "STICK").unwrap()), (0, 0, 0, 0, 4));
    assert_eq!(listing(&usb), before);

    fx.library.remove_track(fx.ids[3]).unwrap();
    assert_eq!(counts(&fx.library.sync_usb_v2(&usb, "STICK").unwrap()), (0, 0, 0, 1, 3));
    assert_eq!(Stick::read(&usb).unwrap().track_count(), 3);
    assert_eq!(sticks::read_manifest(&usb).unwrap().unwrap().files.len(), 3);
}

#[test]
fn a_stick_from_the_old_sync_is_taken_over_without_losing_or_doubling_anything() {
    let fx = fixture("music", 4);
    let usb = stick_dir(&fx);
    let target = SyncTarget { volume: usb.clone(), selection: SyncSelection::WholeLibrary };
    let reports = fx.library.prepare_multi_sync(&[target]).unwrap().run(&NoProgress);
    assert_eq!(reports[0].status, StickStatus::Done, "{:?}", reports[0].error);
    let old = Stick::read(&usb).unwrap();
    let old_ids: BTreeMap<String, u32> =
        old.library().unwrap().tracks.iter().map(|track| (track.path.clone(), track.id)).collect();
    let audio_before: BTreeMap<String, (u64, SystemTime)> =
        listing(&usb).into_iter().filter(|(file, _)| file.starts_with("Contents/")).collect();

    assert_eq!(counts(&fx.library.sync_usb_v2(&usb, "STICK").unwrap()), (0, 4, 0, 0, 0));

    let new = Stick::read(&usb).unwrap();
    for model in [new.device_library.as_ref().unwrap(), new.one_library.as_ref().unwrap()] {
        assert_eq!(model.tracks.len(), 4);
        assert_eq!(model.playlists.len(), 1);
        for track in &model.tracks {
            assert_eq!(old_ids.get(&track.path), Some(&track.id));
        }
    }
    let audio_after: BTreeMap<String, (u64, SystemTime)> =
        listing(&usb).into_iter().filter(|(file, _)| file.starts_with("Contents/")).collect();
    assert_eq!(audio_after, audio_before);
    assert_eq!(counts(&fx.library.sync_usb_v2(&usb, "STICK").unwrap()), (0, 0, 0, 0, 4));
}

#[test]
fn a_rekordbox_stick_with_an_empty_one_library_keeps_its_tracks() {
    let theirs = fixture("theirs", 5);
    let usb = stick_dir(&theirs);
    theirs.library.write_usb_v2(&usb, "RBSTICK").unwrap();
    assert!(!usb.join(MANIFEST_REL_PATH).exists());
    let their_model = Stick::read(&usb).unwrap().device_library.unwrap();
    let emptied = StickModel { tracks: Vec::new(), playlists: Vec::new(), ..their_model.clone() };
    one_library::write(&usb.join("PIONEER/rekordbox/exportLibrary.db"), &emptied).unwrap();
    let their_files = listing(&usb);

    let ours = fixture("ours", 8);
    ours.library.remove_tracks(&ours.ids[..5]).unwrap();
    assert_eq!(counts(&ours.library.sync_usb_v2(&usb, "IGNORED").unwrap()), (3, 0, 0, 0, 0));

    let stick = Stick::read(&usb).unwrap();
    for model in [stick.device_library.as_ref().unwrap(), stick.one_library.as_ref().unwrap()] {
        assert_eq!(model.tracks.len(), 8);
        assert_eq!(model.playlists.len(), 2);
        for their in &their_model.tracks {
            let kept = model.track(their.id).unwrap();
            assert_eq!((&kept.path, &kept.title), (&their.path, &their.title));
        }
    }
    assert_eq!(stick.library().unwrap().device.name, "RBSTICK");
    let now = listing(&usb);
    for (file, stamp) in their_files.iter().filter(|(file, _)| !file.starts_with("PIONEER/rekordbox/")) {
        assert_eq!(now.get(file), Some(stamp), "{file} was touched");
    }
    assert_eq!(sticks::read_manifest(&usb).unwrap().unwrap().files.len(), 3);

    ours.library.remove_tracks(&ours.ids[5..]).unwrap();
    assert_eq!(counts(&ours.library.sync_usb_v2(&usb, "IGNORED").unwrap()), (0, 0, 0, 3, 0));
    let stick = Stick::read(&usb).unwrap();
    assert_eq!(stick.track_count(), 5);
    for their in &their_model.tracks {
        assert!(stick.audio_path(their).is_file());
    }
}
