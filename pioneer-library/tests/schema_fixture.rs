mod support;

use std::path::{Path, PathBuf};

use pioneer_library::failure::{LibraryFailure, classify};
use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{CuePoint, Track};

const FIXTURE: &str = "tests/fixtures/library-schema-v7-beta1";
const FIXTURE_SCHEMA_VERSION: u32 = 7;

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
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

fn copy_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_path(), dir.path());
    let path = dir.path().join("library.db");
    (dir, path)
}

fn stored_version(path: &Path) -> u32 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap()
}

#[test]
#[ignore = "regenerates the committed fixture; run only for a deliberate schema bump"]
fn regenerate_fixture() {
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
        let folder = library.create_folder("Sets", 0).unwrap();
        let playlist = library.create_playlist_in("Warmup", folder as u32).unwrap();
        library.set_playlist_tracks(playlist, &ids).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.query_row("PRAGMA journal_mode=DELETE", [], |_| Ok(())).unwrap();
    conn.execute_batch("VACUUM").unwrap();
    drop(conn);
    assert_eq!(stored_version(&path), FIXTURE_SCHEMA_VERSION);
    for entry in std::fs::read_dir(dir.path().join("anlz")).unwrap().flatten() {
        std::fs::remove_file(entry.path().join("ANLZ0000.EXT")).unwrap();
    }
    let _ = std::fs::remove_dir_all(fixture_path());
    copy_tree(dir.path(), &fixture_path());
}

#[test]
fn fixture_written_by_beta_1_opens_with_data_intact() {
    let (_dir, path) = copy_fixture();
    let library = LocalLibrary::open(&path).expect("the beta 1 library opens");
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

#[test]
fn fixture_survives_a_second_open_and_further_writes() {
    let (_dir, path) = copy_fixture();
    drop(LocalLibrary::open(&path).unwrap());
    let library = LocalLibrary::open(&path).unwrap();
    library.add_track(&track("Delta")).unwrap();
    assert_eq!(library.track_count().unwrap(), 4);
}

#[test]
fn fixture_from_a_newer_schema_is_refused_with_a_typed_error_and_left_alone() {
    let (_dir, path) = copy_fixture();
    let newer = FIXTURE_SCHEMA_VERSION + 1;
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
