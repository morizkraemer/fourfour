mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::Result;
use pioneer_library::sticks::{self, MANIFEST_REL_PATH};
use pioneer_library::{
    LocalLibrary, NoProgress, PreparedMultiSync, ProgressSink, StickProgress, StickStatus, SyncEngine, SyncPhase,
    SyncSelection, SyncTarget,
};
use pioneer_usb_writer::stick::publish::{self, STAGE_DIR};
use pioneer_usb_writer::stick::{one_library, Stick, StickModel};
use tempfile::TempDir;

#[derive(Default)]
struct Recorder {
    progress: Mutex<Vec<StickProgress>>,
}

impl ProgressSink for Recorder {
    fn progress(&self, progress: &StickProgress) {
        self.progress.lock().unwrap().push(progress.clone());
    }
}

struct Fixture {
    _workspace: TempDir,
    root: PathBuf,
    library: LocalLibrary,
    ids: Vec<i64>,
    warmup: i64,
}

fn fixture(folder: &str, track_count: usize) -> Fixture {
    let workspace = TempDir::new().unwrap();
    let root = workspace.path().to_path_buf();
    let library = LocalLibrary::open(&root.join("library.db")).unwrap();
    let ids = support::populate_library(&library, &root.join(folder), track_count, 0.2);
    let warmup = library.create_playlist("Warmup").unwrap();
    library.set_playlist_tracks(warmup, &ids[..2]).unwrap();
    Fixture { _workspace: workspace, root, library, ids, warmup }
}

fn stick(fx: &Fixture, name: &str) -> PathBuf {
    let dir = fx.root.join("volumes").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn whole(volume: &Path) -> SyncTarget {
    SyncTarget { volume: volume.to_path_buf(), selection: SyncSelection::WholeLibrary }
}

fn v2(fx: &Fixture, targets: &[SyncTarget]) -> PreparedMultiSync {
    fx.library.prepare_multi_sync_with(targets, SyncEngine::V2).unwrap()
}

fn plenty(_: &Path) -> Result<u64> {
    Ok(u64::MAX / 2)
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
fn the_engine_is_the_old_writer_unless_asked_for_v2() {
    assert_eq!(SyncEngine::from_setting(None), SyncEngine::Legacy);
    assert_eq!(SyncEngine::from_setting(Some("")), SyncEngine::Legacy);
    assert_eq!(SyncEngine::from_setting(Some("legacy")), SyncEngine::Legacy);
    assert_eq!(SyncEngine::from_setting(Some(" V2 ")), SyncEngine::V2);
    assert_eq!(SyncEngine::default(), SyncEngine::Legacy);

    let fx = fixture("music", 3);
    let usb = stick(&fx, "OLD");
    let prepared = fx.library.prepare_multi_sync_with(&[whole(&usb)], SyncEngine::Legacy).unwrap();
    assert_eq!(prepared.engine(), SyncEngine::Legacy);
    assert_eq!(prepared.run(&NoProgress)[0].status, StickStatus::Done);
    assert!(sticks::read_manifest(&usb).unwrap().unwrap().v2.is_none());
    assert!(!usb.join(STAGE_DIR).exists());
}

#[test]
fn v2_plan_writes_nothing_and_counts_staging_space() {
    let fx = fixture("music", 4);
    let usb = stick(&fx, "A");
    let prepared = v2(&fx, &[whole(&usb)]);

    let plan = prepared.plan_with(&plenty).remove(0).unwrap();
    assert!(std::fs::read_dir(&usb).unwrap().next().is_none());
    assert_eq!((plan.add.len(), plan.remove.len(), plan.update_count, plan.unchanged_count), (4, 0, 0, 0));
    let mut planned_ids: Vec<i64> = plan.add.iter().map(|track| track.track_id.unwrap()).collect();
    planned_ids.sort();
    assert_eq!(planned_ids, fx.ids);
    assert_eq!(plan.bytes_to_add, plan.add.iter().map(|track| track.size_bytes.unwrap()).sum::<u64>());
    assert!(plan.bytes_needed >= plan.bytes_to_add + 16 * 1024 * 1024 + 4 * (128 + 8) * 1024);
    assert_eq!(plan.fits, Some(true));

    let tight = |_: &Path| -> Result<u64> { Ok(1024) };
    assert_eq!(prepared.plan_with(&tight).remove(0).unwrap().fits, Some(false));
    let report = prepared.run_with(&NoProgress, &tight, |_, _, _, _, _| unreachable!()).remove(0);
    assert_eq!(report.status, StickStatus::Failed);
    assert!(report.error.unwrap().contains("Not enough space on A"));
    assert!(std::fs::read_dir(&usb).unwrap().next().is_none());
}

#[test]
fn v2_run_writes_both_databases_reports_progress_and_then_has_nothing_to_do() {
    let fx = fixture("music", 4);
    let usb = stick(&fx, "A");
    let prepared = v2(&fx, &[whole(&usb)]);
    let recorder = Recorder::default();
    let report = prepared.run_with(&recorder, &plenty, |_, _, _, _, _| unreachable!()).remove(0);
    assert_eq!(report.status, StickStatus::Done, "{:?}", report.error);
    assert_eq!(report.report.as_ref().unwrap().tracks_added, 4);

    let on_stick = Stick::read(&usb).unwrap();
    for model in [on_stick.device_library.as_ref().unwrap(), on_stick.one_library.as_ref().unwrap()] {
        assert_eq!((model.tracks.len(), model.playlists.len()), (4, 1));
        assert_eq!(model.playlist_tracks(model.playlists[0].id).len(), 2);
    }
    let manifest = sticks::read_manifest(&usb).unwrap().unwrap();
    assert_eq!((manifest.files.len(), manifest.v2.as_ref().unwrap().paths.len()), (4, 4));
    assert!(!publish::pending(&usb));

    let phases: Vec<SyncPhase> = recorder.progress.lock().unwrap().iter().map(|p| p.phase).collect();
    let first = |phase: SyncPhase| phases.iter().position(|p| *p == phase).unwrap();
    assert!(first(SyncPhase::CheckingSpace) < first(SyncPhase::Copying));
    assert!(first(SyncPhase::Copying) < first(SyncPhase::WritingDatabase));
    assert_eq!(phases.last(), Some(&SyncPhase::Done));
    let copied = recorder.progress.lock().unwrap().iter().filter(|p| p.phase == SyncPhase::WritingDatabase).map(|p| p.done).max();
    assert_eq!(copied, Some(4));

    let before = listing(&usb);
    let again = v2(&fx, &[whole(&usb)]);
    let plan = again.plan_with(&plenty).remove(0).unwrap();
    assert_eq!((plan.add.len(), plan.unchanged_count, plan.bytes_needed), (0, 4, 0));
    let report = again.run_with(&NoProgress, &plenty, |_, _, _, _, _| unreachable!()).remove(0);
    assert_eq!(report.report.unwrap().tracks_unchanged, 4);
    assert_eq!(listing(&usb), before);
}

#[test]
fn v2_playlist_selection_removes_what_is_no_longer_chosen() {
    let fx = fixture("music", 5);
    let usb = stick(&fx, "A");
    assert_eq!(v2(&fx, &[whole(&usb)]).run(&NoProgress)[0].status, StickStatus::Done);

    let only_warmup = SyncTarget {
        volume: usb.clone(),
        selection: SyncSelection::Playlists { playlist_ids: vec![fx.warmup], track_ids: vec![fx.ids[4]] },
    };
    let prepared = v2(&fx, &[only_warmup]);
    let plan = prepared.plan_with(&plenty).remove(0).unwrap();
    assert_eq!((plan.add.len(), plan.remove.len(), plan.unchanged_count), (0, 2, 3));
    let mut removed: Vec<i64> = plan.remove.iter().map(|track| track.track_id.unwrap()).collect();
    removed.sort();
    assert_eq!(removed, vec![fx.ids[2], fx.ids[3]]);
    assert!(plan.bytes_to_free > 0);

    let report = prepared.run(&NoProgress).remove(0);
    assert_eq!(report.report.unwrap().tracks_removed, 2);
    assert_eq!(Stick::read(&usb).unwrap().track_count(), 3);
    assert_eq!(sticks::read_manifest(&usb).unwrap().unwrap().files.len(), 3);
}

#[test]
fn v2_keeps_a_foreign_library_and_says_so_in_the_plan() {
    let theirs = fixture("theirs", 5);
    let usb = stick(&theirs, "RB");
    theirs.library.write_usb_v2(&usb, "RB").unwrap();
    let their_model = Stick::read(&usb).unwrap().device_library.unwrap();
    let emptied = StickModel { tracks: Vec::new(), playlists: Vec::new(), ..their_model.clone() };
    one_library::write(&usb.join("PIONEER/rekordbox/exportLibrary.db"), &emptied).unwrap();
    assert!(!usb.join(MANIFEST_REL_PATH).exists());

    let ours = fixture("ours", 8);
    ours.library.remove_tracks(&ours.ids[..5]).unwrap();
    let prepared = v2(&ours, &[whole(&usb)]);
    let plan = prepared.plan_with(&plenty).remove(0).unwrap();
    assert_eq!((plan.add.len(), plan.foreign_kept_count, plan.remove.len()), (3, 5, 0));
    assert_eq!(prepared.run(&NoProgress)[0].status, StickStatus::Done);

    let on_stick = Stick::read(&usb).unwrap();
    for model in [on_stick.device_library.as_ref().unwrap(), on_stick.one_library.as_ref().unwrap()] {
        assert_eq!(model.tracks.len(), 8);
        for their in &their_model.tracks {
            assert_eq!(model.track(their.id).unwrap().path, their.path);
        }
    }
}

#[test]
fn v2_syncs_two_sticks_at_once_and_repairs_an_interrupted_one_first() {
    let fx = fixture("music", 6);
    let (a, b) = (stick(&fx, "A"), stick(&fx, "B"));
    std::fs::create_dir_all(b.join(STAGE_DIR).join("PIONEER/rekordbox")).unwrap();
    std::fs::write(b.join(STAGE_DIR).join("PIONEER/rekordbox/export.pdb"), b"half written").unwrap();
    assert!(publish::pending(&b));

    let reports = v2(&fx, &[whole(&a), whole(&b)]).run(&NoProgress);
    for report in &reports {
        assert_eq!(report.status, StickStatus::Done, "{:?}", report.error);
        assert_eq!(report.report.as_ref().unwrap().tracks_added, 6);
    }
    let paths = |usb: &Path| -> Vec<String> {
        let mut paths: Vec<String> = Stick::read(usb).unwrap().library().unwrap().tracks.iter().map(|t| t.path.clone()).collect();
        paths.sort();
        paths
    };
    assert_eq!(paths(&a), paths(&b));
    assert_eq!(paths(&a).len(), 6);
    assert!(!publish::pending(&a) && !publish::pending(&b));
}
