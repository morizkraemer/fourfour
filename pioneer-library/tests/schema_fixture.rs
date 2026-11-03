mod support;

use std::path::{Path, PathBuf};

use pioneer_library::failure::{LibraryFailure, classify};
use pioneer_library::{COVERS_DIR, LocalLibrary, SCHEMA_VERSION, normalise_tag};
use pioneer_usb_writer::models::{CuePoint, Track};

const FIXTURE_V7: &str = "tests/fixtures/library-schema-v7-beta1";
const FIXTURE_V8: &str = "tests/fixtures/library-schema-v8";
const LEGACY_TABLE: &str = "artwork_v7_blobs";

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn track(title: &str) -> Track {
    Track {
        source_path: PathBuf::from(format!("/music/{title}.mp3")),
        usb_path: format!("/Contents/Artist/{title}.mp3"),
        title: title.into(),
        artist: "Fixture Artist".into(),
        album: "Fixture Album".into(),
        genre: "House".into(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2026,
        disc_number: 1,
        track_number: 1,
        tempo: 12400,
        key: "8A".into(),
        duration_secs: 240.0,
        sample_rate: 44_100,
        bitrate: 320,
        file_size: 9_000_000,
        id: 0,
        artwork: None,
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_path(name), dir.path());
    let path = dir.path().join("library.db");
    (dir, path)
}

fn stored_version(path: &Path) -> u32 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap()
}

fn cover(n: u8) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
    bytes.extend(std::iter::repeat_n(n, 5000 + n as usize));
    bytes
}

/// The v7 fixture with covers in its blob table, as a v7 build stores them: tracks 1 and 2 share an image.
fn v7_with_covers() -> (tempfile::TempDir, PathBuf) {
    let (dir, path) = copy_fixture(FIXTURE_V7);
    let conn = rusqlite::Connection::open(&path).unwrap();
    for (id, image) in [(1, cover(1)), (2, cover(1)), (3, cover(3))] {
        conn.execute("INSERT INTO artwork (track_id, image_data) VALUES (?1, ?2)", rusqlite::params![id, image]).unwrap();
    }
    (dir, path)
}

fn table_exists(path: &Path, table: &str) -> bool {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |r| r.get(0))
        .unwrap()
}

fn cover_files(dir: &Path) -> usize {
    let mut count = 0;
    for shard in std::fs::read_dir(dir.join(COVERS_DIR)).into_iter().flatten().flatten() {
        count += std::fs::read_dir(shard.path()).unwrap().flatten().filter(|f| !f.file_name().to_string_lossy().starts_with('.')).count();
    }
    count
}

fn stored_keys(path: &Path) -> Vec<(String, String)> {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn.prepare("SELECT artist_key, title_key FROM tracks ORDER BY id").unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
}

fn assert_fixture_data(library: &LocalLibrary) {
    let tracks = library.get_all_tracks().unwrap();
    let titles: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Alpha", "Bravo", "Charlie"]);
    for t in &tracks {
        let analysis = library.get_analysis(t.id as i64).unwrap().expect("analysis row");
        assert_eq!(analysis.bpm, 128.0);
        assert_eq!(analysis.beat_grid.beats.len(), 2);
        let cues: Vec<u32> = analysis.cue_points.iter().map(|c| c.time_ms).collect();
        assert_eq!(cues, [1000, 60_000]);
    }
    let playlists = library.get_all_playlists().unwrap();
    assert_eq!(playlists.len(), 2);
    let warmup = playlists.iter().find(|p| p.name == "Warmup").expect("playlist");
    assert_eq!(warmup.track_ids.len(), 3);
    assert!(playlists.iter().any(|p| p.name == "Sets" && p.is_folder));
}

fn assert_fixture_covers(library: &LocalLibrary) {
    assert_eq!(library.get_artwork(1).unwrap(), Some(cover(1)));
    assert_eq!(library.get_artwork(2).unwrap(), Some(cover(1)));
    assert_eq!(library.get_artwork(3).unwrap(), Some(cover(3)));
    assert!(library.get_track_list_rows().unwrap().iter().all(|row| row.has_artwork));
}

#[test]
#[ignore = "regenerates the committed v8 fixture; run only for a deliberate schema bump"]
fn regenerate_v8_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.db");
    {
        let library = LocalLibrary::open(&path).unwrap();
        let ids: Vec<i64> = ["Alpha", "Bravo", "Charlie"]
            .iter()
            .map(|title| library.add_track(&track(title)).unwrap())
            .collect();
        for id in &ids {
            let mut analysis = support::make_analysis();
            analysis.cue_points = vec![
                CuePoint { hot_cue_number: 1, time_ms: 1000, loop_time_ms: None, ..Default::default() },
                CuePoint { hot_cue_number: 2, time_ms: 60_000, loop_time_ms: Some(64_000), ..Default::default() },
            ];
            library.set_analysis(*id, &analysis).unwrap();
        }
        library.set_artwork(ids[0], &cover(1)).unwrap();
        library.set_artwork(ids[1], &cover(1)).unwrap();
        library.set_artwork(ids[2], &cover(3)).unwrap();
        let folder = library.create_folder("Sets", 0).unwrap();
        let playlist = library.create_playlist_in("Warmup", folder as u32).unwrap();
        library.set_playlist_tracks(playlist, &ids).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.query_row("PRAGMA journal_mode=DELETE", [], |_| Ok(())).unwrap();
    conn.execute_batch("VACUUM").unwrap();
    drop(conn);
    assert_eq!(stored_version(&path), 8);
    for entry in std::fs::read_dir(dir.path().join("anlz")).unwrap().flatten() {
        std::fs::remove_file(entry.path().join("ANLZ0000.EXT")).unwrap();
    }
    let _ = std::fs::remove_dir_all(fixture_path(FIXTURE_V8));
    copy_tree(dir.path(), &fixture_path(FIXTURE_V8));
}

#[test]
fn fixture_written_by_beta_1_opens_with_data_intact() {
    let (_dir, path) = copy_fixture(FIXTURE_V7);
    let library = LocalLibrary::open(&path).expect("the beta 1 library opens");
    assert_fixture_data(&library);
    assert_eq!(stored_version(&path), SCHEMA_VERSION);
}

#[test]
fn fixture_survives_a_second_open_and_further_writes() {
    let (_dir, path) = copy_fixture(FIXTURE_V7);
    drop(LocalLibrary::open(&path).unwrap());
    let library = LocalLibrary::open(&path).unwrap();
    library.add_track(&track("Delta")).unwrap();
    assert_eq!(library.track_count().unwrap(), 4);
}

#[test]
fn v7_migrates_to_v8_with_keys_filled_and_covers_moved_out_of_the_database() {
    let (dir, path) = v7_with_covers();
    let library = LocalLibrary::open(&path).unwrap();
    assert_eq!(stored_version(&path), 8);
    let expected_keys: Vec<(String, String)> =
        ["Alpha", "Bravo", "Charlie"].iter().map(|t| (normalise_tag("Fixture Artist"), normalise_tag(t))).collect();
    assert_eq!(stored_keys(&path), expected_keys);

    assert_eq!(library.legacy_covers().unwrap().map(|(rows, _)| rows), Some(3), "the open does not touch the blobs");
    assert_fixture_covers(&library);
    assert_eq!(cover_files(dir.path()), 0);

    let moved = library.move_legacy_covers(32).unwrap();
    assert_eq!((moved.moved, moved.remaining), (3, 0));
    assert_eq!(library.legacy_covers().unwrap(), None);
    assert!(!table_exists(&path, LEGACY_TABLE));
    assert_eq!(cover_files(dir.path()), 2, "tracks 1 and 2 share one file");
    assert_fixture_covers(&library);
    assert_fixture_data(&library);
    assert_eq!(library.move_legacy_covers(32).unwrap().moved, 0);
}

#[test]
fn the_v8_migration_is_idempotent() {
    let (dir, path) = v7_with_covers();
    {
        let library = LocalLibrary::open(&path).unwrap();
        library.move_legacy_covers(1).unwrap();
    }
    rusqlite::Connection::open(&path).unwrap().execute("UPDATE schema_version SET version = 7", []).unwrap();
    let library = LocalLibrary::open(&path).unwrap();
    assert_eq!(stored_version(&path), 8);
    assert_eq!(library.legacy_covers().unwrap().map(|(rows, _)| rows), Some(2));
    assert_fixture_covers(&library);
    while library.move_legacy_covers(32).unwrap().remaining > 0 {}
    drop(library);

    rusqlite::Connection::open(&path).unwrap().execute("UPDATE schema_version SET version = 7", []).unwrap();
    let library = LocalLibrary::open(&path).unwrap();
    assert_fixture_covers(&library);
    assert_fixture_data(&library);
    assert_eq!(cover_files(dir.path()), 2);
    assert_eq!(stored_keys(&path).len(), 3);
}

#[test]
fn an_interrupted_cover_move_resumes_where_it_stopped() {
    let (dir, path) = v7_with_covers();
    {
        let library = LocalLibrary::open(&path).unwrap();
        assert_eq!(library.move_legacy_covers(1).unwrap().remaining, 2);
    }
    let covers = dir.path().join(COVERS_DIR);
    let orphan = "ab".repeat(32);
    std::fs::create_dir_all(covers.join("ab")).unwrap();
    std::fs::write(covers.join("ab").join(&orphan), b"written by a batch that never committed").unwrap();
    std::fs::write(covers.join("ab").join(".tmp-1-1"), b"half").unwrap();

    let library = LocalLibrary::open(&path).unwrap();
    assert_fixture_covers(&library);
    let done = library.move_legacy_covers(32).unwrap();
    assert_eq!((done.moved, done.remaining), (2, 0));
    assert_fixture_covers(&library);
    assert!(!covers.join("ab").join(&orphan).exists(), "the sweep after the move removes unreferenced files");
    assert!(!covers.join("ab").join(".tmp-1-1").exists());
    assert_eq!(cover_files(dir.path()), 2);
}

#[test]
fn a_cover_set_or_removed_during_the_move_is_kept() {
    let (_dir, path) = v7_with_covers();
    let library = LocalLibrary::open(&path).unwrap();
    library.set_artwork(2, &cover(9)).unwrap();
    library.remove_artwork(3).unwrap();
    while library.move_legacy_covers(32).unwrap().remaining > 0 {}
    assert_eq!(library.get_artwork(1).unwrap(), Some(cover(1)));
    assert_eq!(library.get_artwork(2).unwrap(), Some(cover(9)));
    assert_eq!(library.get_artwork(3).unwrap(), None);
}

#[test]
fn without_space_for_the_covers_they_stay_in_the_database() {
    let (_dir, path) = v7_with_covers();
    let library = LocalLibrary::open(&path).unwrap();
    let err = library.check_space_for_legacy_covers(&|_| Ok(1_000_000)).unwrap_err();
    assert_eq!(classify(&err), Some(LibraryFailure::DiskFull), "{err:#}");
    library.check_space_for_legacy_covers(&|_| Ok(u64::MAX)).unwrap();
    assert_eq!(library.legacy_covers().unwrap().map(|(rows, _)| rows), Some(3));
    assert_fixture_covers(&library);
}

#[test]
fn the_v8_fixture_opens_with_data_and_covers_intact() {
    let (dir, path) = copy_fixture(FIXTURE_V8);
    let library = LocalLibrary::open(&path).unwrap();
    assert_fixture_data(&library);
    assert_fixture_covers(&library);
    assert_eq!(library.legacy_covers().unwrap(), None);
    assert_eq!(cover_files(dir.path()), 2);
}

#[test]
fn fixture_from_a_newer_schema_is_refused_with_a_typed_error_and_left_alone() {
    for fixture in [FIXTURE_V7, FIXTURE_V8] {
        let (_dir, path) = copy_fixture(fixture);
        let newer = SCHEMA_VERSION + 1;
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute("UPDATE schema_version SET version = ?1", [newer])
            .unwrap();
        let err = LocalLibrary::open(&path).err().expect("newer schema must not open");
        assert_eq!(classify(&err), Some(LibraryFailure::NewerVersion), "{err:#}");
        let conn = rusqlite::Connection::open(&path).unwrap();
        let tracks: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0)).unwrap();
        assert_eq!((stored_version(&path), tracks), (newer, 3));
    }
}
