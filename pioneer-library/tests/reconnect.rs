mod support;

use std::path::{Path, PathBuf};

use pioneer_library::rekordbox::ImportedCue;
use pioneer_library::{ChoiceReason, FileStatus, LocalLibrary, file_status, fingerprint_files, plan_reconnect};
use pioneer_usb_writer::scanner;

const SECONDS: f32 = 2.0;

struct Fixture {
    dir: tempfile::TempDir,
    library: LocalLibrary,
    music: PathBuf,
    ids: Vec<i64>,
}

impl Fixture {
    fn new(count: usize) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let ids = support::populate_library(&library, &music, count, SECONDS);
        let fixture = Self { dir, library, music, ids };
        fixture.fingerprint();
        fixture
    }

    fn fingerprint(&self) {
        let targets = self.library.fingerprint_targets().unwrap();
        self.library.store_fingerprints(&fingerprint_files(&targets)).unwrap();
    }

    fn path(&self, index: usize) -> PathBuf {
        self.library.track_files().unwrap().into_iter().find(|f| f.track_id == self.ids[index]).unwrap().path
    }

    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.dir.path().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn plan(&self, folders: &[&Path]) -> pioneer_library::ReconnectPlan {
        let folders: Vec<PathBuf> = folders.iter().map(|p| p.to_path_buf()).collect();
        plan_reconnect(&self.library.reconnect_inputs().unwrap(), &folders)
    }
}

fn cue(slot: u32, time_ms: u32) -> ImportedCue {
    ImportedCue { hot_cue_number: slot, time_ms, loop_time_ms: None, color: None, name: format!("cue {slot}") }
}

#[test]
fn a_moved_and_renamed_file_reconnects_with_cues_analysis_and_playlists() {
    let fx = Fixture::new(3);
    let cues = vec![cue(1, 1000)];
    fx.library.set_cues(fx.ids[0], &cues).unwrap();
    let playlist = fx.library.create_playlist("Peak").unwrap();
    fx.library.set_playlist_tracks(playlist, &fx.ids).unwrap();
    let bpm_before = fx.library.get_analysis(fx.ids[0]).unwrap().unwrap().bpm;

    let elsewhere = fx.folder("elsewhere");
    let moved = elsewhere.join("renamed once.wav");
    std::fs::rename(fx.path(0), &moved).unwrap();

    let plan = fx.plan(&[&elsewhere]);
    assert_eq!(plan.reconnect.len(), 1);
    assert_eq!(plan.reconnect[0].track_id, fx.ids[0]);
    assert!(plan.needs_choice.is_empty() && plan.still_missing.is_empty());

    let picks: Vec<_> = plan.reconnect.iter().map(|r| (r.track_id, PathBuf::from(&r.source_path))).collect();
    fx.library.apply_reconnects(&picks).unwrap();

    assert_eq!(fx.path(0), moved);
    assert_eq!(file_status(&moved), FileStatus::Ok);
    assert_eq!(fx.library.get_cues(fx.ids[0]).unwrap(), cues);
    assert_eq!(fx.library.get_analysis(fx.ids[0]).unwrap().unwrap().bpm, bpm_before);
    let ids_u32: Vec<u32> = fx.ids.iter().map(|id| *id as u32).collect();
    assert_eq!(fx.library.get_playlist(playlist).unwrap().unwrap().track_ids, ids_u32);
    assert_eq!(fx.library.track_count().unwrap(), 3);
}

#[test]
fn a_file_with_the_same_size_but_other_audio_is_not_the_track() {
    let fx = Fixture::new(2);
    let elsewhere = fx.folder("elsewhere");
    let lost = fx.path(0);
    std::fs::remove_file(&lost).unwrap();
    support::write_wav(&elsewhere.join("impostor.wav"), 997.0, SECONDS);

    let plan = fx.plan(&[&elsewhere]);

    assert!(plan.reconnect.is_empty() && plan.needs_choice.is_empty());
    assert_eq!(plan.still_missing, vec![fx.ids[0]]);
}

#[test]
fn a_copy_with_the_original_still_in_place_is_not_a_reconnect() {
    let fx = Fixture::new(2);
    let elsewhere = fx.folder("elsewhere");
    std::fs::copy(fx.path(0), elsewhere.join("copy.wav")).unwrap();

    let plan = fx.plan(&[&elsewhere]);
    assert_eq!(plan, pioneer_library::ReconnectPlan::default());

    let err = fx.library.apply_reconnects(&[(fx.ids[0], elsewhere.join("copy.wav"))]).unwrap_err();
    assert!(err.to_string().contains("copy, not a move"), "{err}");
    assert_eq!(file_status(&fx.path(0)), FileStatus::Ok);
}

#[test]
fn a_moved_file_that_is_also_a_copy_of_a_present_track_needs_a_choice() {
    let fx = Fixture::new(1);
    let twin = fx.folder("twin");
    support::write_wav(&twin.join("twin.wav"), 110.0, SECONDS);
    let twin_track = scanner::scan_files(&[twin.join("twin.wav")]).unwrap();
    fx.library.add_tracks(&twin_track).unwrap();
    fx.fingerprint();

    std::fs::remove_file(fx.path(0)).unwrap();
    let elsewhere = fx.folder("elsewhere");
    std::fs::copy(twin.join("twin.wav"), elsewhere.join("found.wav")).unwrap();

    let plan = fx.plan(&[&elsewhere]);

    assert!(plan.reconnect.is_empty());
    assert_eq!(plan.needs_choice.len(), 1);
    assert_eq!(plan.needs_choice[0].track_id, fx.ids[0]);
    assert_eq!(plan.needs_choice[0].reason, ChoiceReason::Copy);
}

#[test]
fn two_candidates_for_one_track_need_a_choice_and_reconnect_nothing() {
    let fx = Fixture::new(2);
    let elsewhere = fx.folder("elsewhere");
    let lost = fx.path(0);
    std::fs::copy(&lost, elsewhere.join("a.wav")).unwrap();
    std::fs::copy(&lost, elsewhere.join("b.wav")).unwrap();
    std::fs::remove_file(&lost).unwrap();

    let plan = fx.plan(&[&elsewhere]);

    assert!(plan.reconnect.is_empty());
    assert_eq!(plan.needs_choice.len(), 1);
    let choice = &plan.needs_choice[0];
    assert_eq!((choice.track_id, choice.reason), (fx.ids[0], ChoiceReason::MultipleCandidates));
    assert_eq!(choice.candidates.len(), 2);

    fx.library.apply_reconnects(&[(fx.ids[0], elsewhere.join("b.wav"))]).unwrap();
    assert_eq!(fx.path(0), elsewhere.join("b.wav"));
}

#[test]
fn one_file_wanted_by_two_missing_tracks_stays_unreconnected() {
    let fx = Fixture::new(1);
    let other = fx.folder("other");
    std::fs::copy(fx.path(0), other.join("same.wav")).unwrap();
    let second = scanner::scan_files(&[other.join("same.wav")]).unwrap();
    let second_id = fx.library.add_tracks(&second).unwrap()[0];
    fx.fingerprint();
    std::fs::remove_file(fx.path(0)).unwrap();
    std::fs::remove_file(other.join("same.wav")).unwrap();
    let elsewhere = fx.folder("elsewhere");
    support::write_wav(&elsewhere.join("only.wav"), 110.0, SECONDS);

    let plan = fx.plan(&[&elsewhere]);

    assert!(plan.reconnect.is_empty());
    let mut ids: Vec<i64> = plan.needs_choice.iter().map(|c| c.track_id).collect();
    ids.sort();
    assert_eq!(ids, vec![fx.ids[0], second_id]);
    assert!(plan.needs_choice.iter().all(|c| c.reason == ChoiceReason::SharedFile));
}

#[test]
fn without_a_stored_hash_a_single_match_is_offered_not_applied() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let ids = support::populate_library(&library, &music, 1, SECONDS);
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let old = library.track_files().unwrap().remove(0).path;
    std::fs::rename(&old, elsewhere.join("track0000.wav")).unwrap();

    let plan = plan_reconnect(&library.reconnect_inputs().unwrap(), &[elsewhere]);

    assert!(plan.reconnect.is_empty());
    assert_eq!(plan.needs_choice.len(), 1);
    assert_eq!((plan.needs_choice[0].track_id, plan.needs_choice[0].reason), (ids[0], ChoiceReason::Unconfirmed));
}

#[test]
fn a_file_already_used_by_a_track_is_never_a_candidate() {
    let fx = Fixture::new(2);
    std::fs::remove_file(fx.path(0)).unwrap();

    let plan = fx.plan(&[&fx.music]);

    assert_eq!(plan.still_missing, vec![fx.ids[0]]);
    let err = fx.library.apply_reconnects(&[(fx.ids[0], fx.path(1))]).unwrap_err();
    assert!(err.to_string().contains("belongs to another track"), "{err}");
}

#[test]
fn relocating_a_folder_remaps_by_relative_path_and_undo_restores_the_old_paths() {
    let fx = Fixture::new(3);
    let cues = vec![cue(1, 1000)];
    fx.library.set_cues(fx.ids[0], &cues).unwrap();
    let before = fx.library.track_files().unwrap();
    let moved = fx.dir.path().join("moved");
    std::fs::rename(&fx.music, &moved).unwrap();
    std::fs::remove_file(moved.join("artist00/track0002.wav")).unwrap();

    let preview = fx.library.preview_remap(&fx.music, &moved).unwrap();
    assert_eq!((preview.total, preview.found, preview.still_missing), (3, 2, 1));
    assert_eq!(fx.library.track_files().unwrap(), before);

    let applied = fx.library.apply_remap(&fx.music, &moved).unwrap();
    assert_eq!((applied.remapped.len(), applied.still_missing), (2, 1));
    let now = fx.library.track_files().unwrap();
    assert_eq!(now[0].path, moved.join("artist00/track0000.wav"));
    assert_eq!(now[1].path, moved.join("artist00/track0001.wav"));
    assert_eq!(now[2].path, before[2].path);
    assert_eq!(fx.library.get_cues(fx.ids[0]).unwrap(), cues);
    assert_eq!(fx.library.last_folder_relocation().unwrap().unwrap().tracks, 2);

    let undone = fx.library.undo_remap(applied.relocation_id).unwrap();
    assert_eq!((undone.reverted.len(), undone.skipped), (2, 0));
    assert_eq!(fx.library.track_files().unwrap(), before);
    assert!(fx.library.last_folder_relocation().unwrap().is_none());
    assert!(fx.library.undo_remap(applied.relocation_id).is_err());
}

#[test]
fn undo_leaves_a_track_alone_that_was_moved_again() {
    let fx = Fixture::new(2);
    let moved = fx.dir.path().join("moved");
    std::fs::rename(&fx.music, &moved).unwrap();
    let applied = fx.library.apply_remap(&fx.music, &moved).unwrap();
    let again = fx.folder("again");
    std::fs::rename(moved.join("artist00/track0001.wav"), again.join("x.wav")).unwrap();
    fx.library.relink_track(fx.ids[1], &again.join("x.wav")).unwrap();

    let undone = fx.library.undo_remap(applied.relocation_id).unwrap();

    assert_eq!((undone.reverted.len(), undone.skipped), (1, 1));
    assert_eq!(fx.path(1), again.join("x.wav"));
    assert_eq!(fx.path(0), fx.music.join("artist00/track0000.wav"));
}

#[test]
fn a_relocation_with_nothing_found_or_to_the_same_folder_is_refused() {
    let fx = Fixture::new(2);
    let empty = fx.folder("empty");

    assert_eq!(fx.library.preview_remap(&fx.music, &empty).unwrap().found, 0);
    assert!(fx.library.apply_remap(&fx.music, &empty).is_err());
    assert!(fx.library.preview_remap(&fx.music, &fx.music).is_err());
    assert!(fx.library.preview_remap(&fx.music, &fx.dir.path().join("nope")).is_err());
}

#[test]
fn a_remap_does_not_take_a_path_another_track_has() {
    let fx = Fixture::new(2);
    let moved = fx.dir.path().join("moved");
    std::fs::create_dir_all(moved.join("artist00")).unwrap();
    std::fs::copy(fx.path(0), moved.join("artist00/track0000.wav")).unwrap();
    std::fs::copy(fx.path(1), moved.join("artist00/track0001.wav")).unwrap();
    let squatter = scanner::scan_files(&[moved.join("artist00/track0001.wav")]).unwrap();
    fx.library.add_tracks(&squatter).unwrap();

    let preview = fx.library.preview_remap(&fx.music, &moved).unwrap();

    assert_eq!((preview.total, preview.found, preview.still_missing), (2, 1, 1));
}
