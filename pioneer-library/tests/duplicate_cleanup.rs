mod support;

use std::path::{Path, PathBuf};

use pioneer_library::rekordbox::{ImportedCue, ImportedMeta, SourceKind};
use pioneer_library::{DuplicateConfidence, LocalLibrary, MergeRequest, find_duplicate_groups};
use pioneer_usb_writer::models::Track;

fn cue(slot: u32, time_ms: u32, name: &str) -> ImportedCue {
    ImportedCue { hot_cue_number: slot, time_ms, loop_time_ms: None, color: None, name: name.into() }
}

struct Fixture {
    dir: tempfile::TempDir,
    library: LocalLibrary,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        Self { dir, library }
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.dir.path().join("music").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn add(&self, name: &str, bytes: &[u8], artist: &str, title: &str, secs: f64, bitrate: u32) -> i64 {
        let path = self.file(name, bytes);
        let track = Track {
            id: 0,
            source_path: path,
            usb_path: format!("/Contents/{name}"),
            title: title.into(),
            artist: artist.into(),
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
            duration_secs: secs,
            sample_rate: 44100,
            bitrate,
            file_size: bytes.len() as u64,
            artwork: None,
        };
        let id = self.library.add_track(&track).unwrap();
        self.library.set_analysis(id, &support::make_analysis()).unwrap();
        id
    }

    fn groups(&self) -> Vec<pioneer_library::DuplicateGroup> {
        find_duplicate_groups(&self.library.duplicate_rows().unwrap())
    }

    fn playlist(&self, name: &str, ids: &[i64]) -> i64 {
        let id = self.library.create_playlist(name).unwrap();
        self.library.set_playlist_tracks(id, ids).unwrap();
        id
    }

    fn playlist_ids(&self, playlist: i64) -> Vec<i64> {
        self.library.get_playlist(playlist).unwrap().unwrap().track_ids.iter().map(|id| *id as i64).collect()
    }

    fn merge(&self, keeper_id: i64, remove_ids: &[i64], require_identical: bool) -> pioneer_library::MergeReport {
        self.library.apply_duplicate_merges(&[MergeRequest { keeper_id, remove_ids: remove_ids.to_vec(), require_identical }])
    }
}

fn lossless_and_mp3(f: &Fixture) -> (i64, i64) {
    let mp3 = f.add("a/Hale - Copper Lines.mp3", b"mp3 bytes of copper lines", "Hale", "Copper Lines", 380.4, 320);
    let flac = f.add("b/Hale - Copper Lines.flac", b"flac bytes of copper lines, a bit longer", "HALE", "Copper Lines (Original Mix)", 380.0, 900);
    (mp3, flac)
}

#[test]
fn lossless_beats_mp3_as_a_metadata_only_group_with_a_reason() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    f.add("c/Other.mp3", b"other", "Hale", "Other Song", 200.0, 320);

    let groups = f.groups();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.confidence, DuplicateConfidence::MetadataOnly);
    assert_eq!(group.keeper_id, flac);
    assert_eq!(group.members.iter().map(|m| m.id).collect::<Vec<_>>(), vec![flac, mp3]);
    assert_eq!(group.reason, "Lossless (FLAC) over lossy (MP3)");
}

#[test]
fn byte_identical_files_are_an_exact_group_and_a_title_match_beyond_two_seconds_is_not_a_group() {
    let f = Fixture::new();
    let first = f.add("a/Song.mp3", b"same bytes", "Ode", "Glass Harbor", 300.0, 320);
    let copy = f.add("b/Song (1).mp3", b"same bytes", "Ode", "Glass Harbor", 300.0, 320);
    f.add("c/Song long.mp3", b"different bytes!", "Ode", "Glass Harbor", 303.5, 320);

    let groups = f.groups();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].confidence, DuplicateConfidence::ExactAudio);
    assert_eq!(groups[0].members.iter().map(|m| m.id).collect::<Vec<_>>(), vec![first, copy]);
    assert_eq!(groups[0].reason, "Same quality everywhere; the first entry");
}

#[test]
fn identical_files_with_different_tags_are_still_exact_and_one_exact_pair_plus_a_third_copy_is_metadata_only() {
    let f = Fixture::new();
    let a = f.add("a/x.mp3", b"same bytes", "Ode", "Glass Harbor", 300.0, 320);
    let b = f.add("b/y.mp3", b"same bytes", "Someone", "Untitled", 12.0, 320);
    let exact = f.groups();
    assert_eq!((exact.len(), exact[0].confidence), (1, DuplicateConfidence::ExactAudio));
    assert_eq!(exact[0].members.len(), 2);
    let c = f.add("c/z.flac", b"flac", "Someone", "Untitled", 12.5, 900);
    let mixed = f.groups();
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].confidence, DuplicateConfidence::MetadataOnly);
    assert_eq!(mixed[0].members.len(), 3);
    assert_eq!(mixed[0].keeper_id, c);
    assert_ne!(a, b);
}

#[test]
fn more_playlists_then_more_cues_then_oldest_pick_the_keeper() {
    let f = Fixture::new();
    let a = f.add("a/s.mp3", b"aaaa", "Ode", "Glass Harbor", 300.0, 320);
    let b = f.add("b/s.mp3", b"bbbb", "Ode", "Glass Harbor", 300.0, 320);
    f.playlist("One", &[b]);
    assert_eq!(f.groups()[0].keeper_id, b);
    assert_eq!(f.groups()[0].reason, "In more playlists (1 vs 0)");
    f.playlist("Two", &[a]);
    f.library.set_cues(a, &[cue(1, 1000, "drop")]).unwrap();
    assert_eq!(f.groups()[0].keeper_id, a);
    assert_eq!(f.groups()[0].reason, "More cues (1 vs 0)");
    f.library.set_cues(b, &[cue(1, 1000, "drop")]).unwrap();
    assert_eq!(f.groups()[0].keeper_id, a);
    assert_eq!(f.groups()[0].reason, "Same quality everywhere; the first entry");
}

#[test]
fn a_missing_file_never_becomes_the_keeper_over_a_present_one() {
    let f = Fixture::new();
    let flac = f.add("a/s.flac", b"flac", "Ode", "Glass Harbor", 300.0, 900);
    let mp3 = f.add("b/s.mp3", b"mp3", "Ode", "Glass Harbor", 300.0, 320);
    std::fs::remove_file(f.dir.path().join("music/a/s.flac")).unwrap();

    let group = &f.groups()[0];

    assert_eq!(group.keeper_id, mp3);
    assert!(!group.members.iter().find(|m| m.id == flac).unwrap().file_present);
}

#[test]
fn merging_moves_playlist_places_to_the_keeper_and_keeps_every_file() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    let other = f.add("c/Other.mp3", b"other", "Hale", "Other Song", 200.0, 320);
    let only_mp3 = f.playlist("Only mp3", &[other, mp3]);
    let both = f.playlist("Both", &[mp3, flac, other]);
    let only_flac = f.playlist("Only flac", &[flac]);
    let mp3_path = f.library.get_track(mp3).unwrap().unwrap().source_path;

    let report = f.merge(flac, &[mp3], false);

    assert_eq!(report.failed, vec![]);
    assert_eq!(report.merged.len(), 1);
    assert!(f.library.get_track(mp3).unwrap().is_none());
    assert_eq!(f.playlist_ids(only_mp3), vec![other, flac]);
    assert_eq!(f.playlist_ids(both), vec![flac, other]);
    assert_eq!(f.playlist_ids(only_flac), vec![flac]);
    assert!(mp3_path.exists());
    assert!(f.groups().is_empty());
}

#[test]
fn merging_fills_the_keepers_empty_cue_slots_rating_colour_and_comment_and_keeps_its_own_values() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    f.library.set_cues(flac, &[cue(1, 1000, "keeper A")]).unwrap();
    f.library.set_cues(mp3, &[cue(1, 5000, "other A"), cue(2, 2000, "other B"), cue(0, 1020, "near memory"), cue(0, 9000, "far memory")]).unwrap();
    f.library.set_user_rating(mp3, 4).unwrap();
    f.library.set_user_rating(flac, 2).unwrap();
    let mut track = f.library.get_track(mp3).unwrap().unwrap();
    track.comment = "from the mp3".into();
    f.library.update_track(mp3, &track).unwrap();

    f.merge(flac, &[mp3], false);

    let cues = f.library.get_cues(flac).unwrap();
    let described: Vec<(u32, u32, &str)> = cues.iter().map(|c| (c.hot_cue_number, c.time_ms, c.name.as_str())).collect();
    assert_eq!(described, vec![(1, 1000, "keeper A"), (2, 2000, "other B"), (0, 1020, "near memory"), (0, 9000, "far memory")]);
    assert_eq!(f.library.effective_rating(flac, None), Some(2));
    assert_eq!(f.library.get_track(flac).unwrap().unwrap().comment, "from the mp3");
}

#[test]
fn an_unrated_keeper_takes_the_rating_of_the_merged_copy() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    f.library.set_user_rating(mp3, 5).unwrap();

    f.merge(flac, &[mp3], false);

    assert_eq!(f.library.effective_rating(flac, None), Some(5));
}

#[test]
fn restoring_a_merge_puts_back_ids_rows_playlists_cues_and_the_keepers_old_values() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    let mp3_cues = vec![cue(1, 5000, "other A"), cue(2, 2000, "other B")];
    f.library.set_cues(mp3, &mp3_cues).unwrap();
    f.library.set_user_rating(mp3, 4).unwrap();
    let only_mp3 = f.playlist("Only mp3", &[mp3]);
    let both = f.playlist("Both", &[flac, mp3]);
    let mp3_row_before = f.library.get_track(mp3).unwrap().unwrap();
    let mp3_analysis_before = f.library.get_analysis(mp3).unwrap().unwrap();
    let flac_cues_before = f.library.get_cues(flac).unwrap();

    let merge_id = f.merge(flac, &[mp3], false).merged[0].merge_id;
    assert_eq!(f.library.list_archive().unwrap().len(), 1);
    assert_eq!(f.library.list_archive().unwrap()[0].tracks[0].id, mp3);
    let outcome = f.library.restore_merge(merge_id).unwrap();

    assert_eq!(outcome.restored_ids, vec![mp3]);
    assert!(outcome.keeper_reverted);
    let mp3_row_after = f.library.get_track(mp3).unwrap().unwrap();
    let row = |t: &Track| (t.id, t.source_path.clone(), t.title.clone(), t.artist.clone(), t.comment.clone(), t.bitrate, t.file_size, t.duration_secs.to_bits());
    assert_eq!(row(&mp3_row_after), row(&mp3_row_before));
    assert_eq!(f.library.get_analysis(mp3).unwrap().unwrap().bpm, mp3_analysis_before.bpm);
    assert_eq!(f.library.get_cues(mp3).unwrap(), mp3_cues);
    assert_eq!(f.library.effective_rating(mp3, None), Some(4));
    assert_eq!(f.playlist_ids(only_mp3), vec![mp3]);
    assert_eq!(f.playlist_ids(both), vec![flac, mp3]);
    assert_eq!(f.library.get_cues(flac).unwrap(), flac_cues_before);
    assert_eq!(f.library.effective_rating(flac, None), None);
    assert!(f.library.list_archive().unwrap().is_empty());
    assert_eq!(f.groups().len(), 1);
}

#[test]
fn restoring_leaves_a_keeper_that_changed_since_the_merge_alone() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    f.library.set_cues(mp3, &[cue(1, 5000, "other A")]).unwrap();
    let merge_id = f.merge(flac, &[mp3], false).merged[0].merge_id;
    f.library.set_cues(flac, &[cue(1, 5000, "other A"), cue(3, 7000, "added later")]).unwrap();

    let outcome = f.library.restore_merge(merge_id).unwrap();

    assert!(!outcome.keeper_reverted);
    assert_eq!(f.library.get_cues(flac).unwrap().len(), 2);
    assert_eq!(f.library.get_cues(mp3).unwrap().len(), 1);
}

#[test]
fn restore_refuses_when_the_file_is_back_in_the_library_and_changes_nothing() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    let path = f.library.get_track(mp3).unwrap().unwrap();
    let merge_id = f.merge(flac, &[mp3], false).merged[0].merge_id;
    f.library.add_track(&path).unwrap();

    let error = f.library.restore_merge(merge_id).unwrap_err();

    assert!(error.to_string().contains("in the library again"), "{error}");
    assert_eq!(f.library.list_archive().unwrap().len(), 1);
}

#[test]
fn an_exact_merge_refuses_files_that_differ_and_merges_nothing() {
    let f = Fixture::new();
    let a = f.add("a/s.mp3", b"aaaa", "Ode", "Glass Harbor", 300.0, 320);
    let b = f.add("b/s.mp3", b"bbbb", "Ode", "Glass Harbor", 300.0, 320);

    let report = f.merge(a, &[b], true);

    assert_eq!(report.merged, vec![]);
    assert!(report.failed[0].message.contains("not an identical copy"), "{:?}", report.failed);
    assert!(f.library.get_track(b).unwrap().is_some());
    assert!(f.library.list_archive().unwrap().is_empty());
}

#[test]
fn merging_a_group_of_three_archives_all_removed_entries_under_one_restore() {
    let f = Fixture::new();
    let a = f.add("a/s.flac", b"aaaa", "Ode", "Glass Harbor", 300.0, 900);
    let b = f.add("b/s.mp3", b"bbbb", "Ode", "Glass Harbor", 300.0, 320);
    let c = f.add("c/s.mp3", b"cccc", "Ode", "Glass Harbor", 300.5, 192);
    let list = f.playlist("List", &[c, b]);

    let merge_id = f.merge(a, &[b, c], false).merged[0].merge_id;
    assert_eq!(f.playlist_ids(list), vec![a]);
    let archive = f.library.list_archive().unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive[0].tracks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![b, c]);
    assert_eq!(archive[0].keeper_label.as_deref(), Some("Ode - Glass Harbor"));

    f.library.restore_merge(merge_id).unwrap();
    assert_eq!(f.playlist_ids(list), vec![c, b]);
    assert_eq!(f.library.track_count().unwrap(), 3);
}

#[test]
fn a_merge_of_missing_ids_or_the_keeper_into_itself_fails_without_changes() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);

    let report = f.library.apply_duplicate_merges(&[
        MergeRequest { keeper_id: flac, remove_ids: vec![flac], require_identical: false },
        MergeRequest { keeper_id: flac, remove_ids: vec![9999], require_identical: false },
        MergeRequest { keeper_id: flac, remove_ids: vec![], require_identical: false },
    ]);

    assert_eq!(report.failed.len(), 3);
    assert_eq!(f.library.track_count().unwrap(), 2);
    assert_ne!(mp3, flac);
}

#[test]
fn archived_covers_survive_a_cover_sweep_and_come_back_on_restore() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    let cover = vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
    f.library.set_artwork(mp3, &cover).unwrap();

    let merge_id = f.merge(flac, &[mp3], false).merged[0].merge_id;
    f.library.sweep_covers().unwrap();
    f.library.restore_merge(merge_id).unwrap();

    assert_eq!(f.library.get_artwork(mp3).unwrap(), Some(cover));
}

#[test]
fn a_library_saved_before_the_archive_existed_opens_and_merges() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.db");
    {
        let library = LocalLibrary::open(&db).unwrap();
        drop(library);
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("DROP TABLE archived_tracks; DROP TABLE duplicate_merges;").unwrap();
    }

    let library = LocalLibrary::open(&db).unwrap();

    assert!(library.list_archive().unwrap().is_empty());
    assert!(Path::new(&db).exists());
}

#[test]
fn a_colourless_keeper_takes_the_colour_without_locking_its_grid_or_cues_and_restore_removes_it() {
    let f = Fixture::new();
    let (mp3, flac) = lossless_and_mp3(&f);
    let anlz = |id: i64| f.dir.path().join("anlz").join(id.to_string());
    ImportedMeta { source: SourceKind::Xml, source_id: "7".into(), grid_locked: true, cues_locked: false, rating: None, color: Some("green".into()), cues: vec![], grid: vec![] }
        .write(&anlz(mp3))
        .unwrap();

    let merge_id = f.merge(flac, &[mp3], false).merged[0].merge_id;

    let meta = ImportedMeta::read(&anlz(flac)).unwrap();
    assert_eq!(meta.color.as_deref(), Some("green"));
    assert!(!meta.grid_locked && !meta.cues_locked);
    f.library.restore_merge(merge_id).unwrap();
    assert!(ImportedMeta::read(&anlz(flac)).is_none());
    assert_eq!(ImportedMeta::read(&anlz(mp3)).unwrap().color.as_deref(), Some("green"));
}
