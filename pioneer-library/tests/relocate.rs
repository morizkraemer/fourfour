mod support;

use std::path::PathBuf;

use pioneer_library::rekordbox::ImportedCue;
use pioneer_library::{FileStatus, LocalLibrary, file_status};

fn cue(slot: u32, time_ms: u32) -> ImportedCue {
    ImportedCue { hot_cue_number: slot, time_ms, loop_time_ms: None, color: None, name: format!("cue {slot}") }
}

#[test]
fn relocating_a_moved_folder_keeps_cues_grid_and_playlists() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let ids = support::populate_library(&library, &music, 3, 0.2);

    let cues = vec![cue(1, 1000), cue(2, 2500)];
    library.set_cues(ids[0], &cues).unwrap();
    let beats = |lib: &LocalLibrary| -> Vec<u32> { lib.get_analysis(ids[0]).unwrap().unwrap().beat_grid.beats.iter().map(|b| b.time_ms).collect() };
    let grid_before = beats(&library);
    let playlist = library.create_playlist("Peak").unwrap();
    library.set_playlist_tracks(playlist, &ids).unwrap();

    let moved = dir.path().join("moved");
    std::fs::rename(&music, &moved).unwrap();
    let statuses: Vec<FileStatus> = library.track_files().unwrap().iter().map(|f| file_status(&f.path)).collect();
    assert_eq!(statuses, vec![FileStatus::Missing; 3]);

    let report = library.relocate_in_folder(&ids, &moved).unwrap();
    assert_eq!(report.matched.len(), 3);
    assert!(report.ambiguous.is_empty() && report.not_found.is_empty());

    for file in library.track_files().unwrap() {
        assert!(file.path.starts_with(&moved), "{} not relinked", file.path.display());
        assert_eq!(file_status(&file.path), FileStatus::Ok);
    }
    assert_eq!(library.get_cues(ids[0]).unwrap(), cues);
    assert_eq!(beats(&library), grid_before);
    let ids_u32: Vec<u32> = ids.iter().map(|id| *id as u32).collect();
    assert_eq!(library.get_playlist(playlist).unwrap().unwrap().track_ids, ids_u32);
}

#[test]
fn relocate_reports_files_it_cannot_find_and_leaves_them_alone() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let ids = support::populate_library(&library, &music, 2, 0.1);
    let before = library.track_files().unwrap();
    std::fs::remove_dir_all(&music).unwrap();
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    let report = library.relocate_in_folder(&ids, &empty).unwrap();

    assert!(report.matched.is_empty());
    assert_eq!(report.not_found, ids);
    assert_eq!(library.track_files().unwrap(), before);
}

#[test]
fn relink_one_track_checks_the_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let ids = support::populate_library(&library, &dir.path().join("music"), 1, 0.1);
    let elsewhere = dir.path().join("elsewhere/renamed.wav");
    support::write_wav(&elsewhere, 220.0, 0.1);

    assert!(library.relink_track(ids[0], &PathBuf::from("/nope/x.wav")).is_err());
    assert!(library.relink_track(9999, &elsewhere).is_err());
    library.relink_track(ids[0], &elsewhere).unwrap();
    assert_eq!(library.get_track(ids[0]).unwrap().unwrap().source_path, elsewhere);
}
