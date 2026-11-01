#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};

use pioneer_library::{normalise_tag, DuplicateCandidate, DuplicateIndex, FileStatus, LocalLibrary, file_status};
use pioneer_usb_writer::models::Track;

fn track(path: &str) -> Track {
    Track {
        source_path: PathBuf::from(path),
        usb_path: format!("/Contents/Unknown Artist/{}", path.rsplit('/').next().unwrap()),
        title: "t".into(),
        artist: "a".into(),
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
        duration_secs: 1.0,
        sample_rate: 44_100,
        bitrate: 320,
        file_size: 1,
        id: 0,
        artwork: None,
    }
}

fn open() -> (tempfile::TempDir, LocalLibrary) {
    let dir = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    (dir, library)
}

#[test]
fn extreme_field_values_store_and_read_back() {
    let (_dir, library) = open();
    let emoji = "\u{1F3A7}\u{1F525} \u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{645}\u{631}\u{62D}\u{628}\u{627} e\u{301}\u{202E}";
    let mut cases = Vec::new();
    for (i, mutate) in [
        Box::new(|t: &mut Track| t.title = "A".repeat(3 * 1024 * 1024)) as Box<dyn Fn(&mut Track)>,
        Box::new(|t: &mut Track| t.artist = "x\0y".into()),
        Box::new(|t: &mut Track| t.title = emoji.repeat(100)),
        Box::new(|t: &mut Track| {
            t.title.clear();
            t.artist.clear();
        }),
        Box::new(|t: &mut Track| t.sample_rate = 0),
        Box::new(|t: &mut Track| t.bitrate = 0),
        Box::new(|t: &mut Track| t.duration_secs = 0.0),
        Box::new(|t: &mut Track| t.duration_secs = 36_000.0),
        Box::new(|t: &mut Track| t.file_size = u64::MAX),
        Box::new(|t: &mut Track| t.file_size = 0),
        Box::new(|t: &mut Track| t.year = u16::MAX),
        Box::new(|t: &mut Track| t.track_number = u32::MAX),
        Box::new(|t: &mut Track| t.tempo = u32::MAX),
        Box::new(|t: &mut Track| t.usb_path.clear()),
    ]
    .into_iter()
    .enumerate()
    {
        let mut t = track(&format!("/music/case{i}.wav"));
        mutate(&mut t);
        cases.push(t);
    }
    let ids = library.add_tracks(&cases).expect("a batch of extreme rows is accepted");
    assert_eq!(ids.len(), cases.len());
    let all = library.get_all_tracks().unwrap();
    assert_eq!(all.len(), cases.len());
    library.get_track_list_rows().unwrap();
    library.get_track_list_rows_without_previews().unwrap();
    let candidates = library.duplicate_candidates().unwrap();
    let index = DuplicateIndex::new(&candidates);
    for t in &cases {
        index.find(t);
    }
    library.track_files().unwrap();
    for (id, t) in ids.iter().zip(&cases) {
        library.update_track(*id, t).unwrap();
        library.set_analysis(*id, &support::make_analysis()).unwrap();
        assert!(library.get_analysis(*id).unwrap().is_some());
    }
}

#[test]
fn a_batch_with_one_repeated_path_fails_whole_and_leaves_the_library_usable() {
    let (_dir, library) = open();
    library.add_tracks(&[track("/music/keep.wav")]).unwrap();
    let batch = [track("/music/new1.wav"), track("/music/new2.wav"), track("/music/keep.wav")];
    assert!(library.add_tracks(&batch).is_err());
    assert_eq!(library.track_count().unwrap(), 1, "no half-added batch");
    library.add_tracks(&batch[..2]).unwrap();
    assert_eq!(library.track_count().unwrap(), 3);
}

#[test]
fn duplicate_detection_copes_with_odd_text() {
    let big = "\u{1F3A7}".repeat(500_000);
    assert!(normalise_tag(&big).len() <= big.len() * 4);
    for text in ["", " ", "\0", "\u{202E}", "\u{FEFF}", "\u{200B}\u{200B}", "(Original Mix)", " original mix", "\u{FDFA}", "\u{1F1E9}\u{1F1EA}"] {
        normalise_tag(text);
    }
    let candidates: Vec<DuplicateCandidate> = (0..50)
        .map(|i| DuplicateCandidate {
            id: i,
            source_path: PathBuf::from(format!("/music/\u{1F525}{i}.wav")),
            file_size: if i % 2 == 0 { 0 } else { u64::MAX },
            artist: ["", "\0", "\u{202E}x", "Unknown Artist"][i as usize % 4].to_string(),
            title: ["", "\u{FEFF}", "(Original Mix)", "\u{1F3A7}"][i as usize % 4].to_string(),
            duration_secs: [f64::NAN, f64::INFINITY, -1.0, 0.0][i as usize % 4],
        })
        .collect();
    let index = DuplicateIndex::new(&candidates);
    for c in &candidates {
        let mut t = track(&c.source_path.to_string_lossy());
        t.artist = c.artist.clone();
        t.title = c.title.clone();
        t.duration_secs = c.duration_secs;
        index.find(&t);
    }
}

#[test]
fn relocating_in_a_folder_with_loops_and_locked_subfolders_ends() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let music = dir.path().join("music");
    let ids = support::populate_library(&library, &music, 3, 0.2);
    let moved = dir.path().join("moved");
    std::fs::rename(&music, &moved).unwrap();
    symlink(&moved, moved.join("loop")).unwrap();
    symlink("../moved", moved.join("artist00").join("up")).unwrap();
    let locked = moved.join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let report = library.relocate_in_folder(&ids, &moved).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(report.matched.len(), 3, "{report:?}");
}

#[test]
fn file_status_handles_paths_that_cannot_exist() {
    assert_eq!(file_status(Path::new("/definitely/not/here.wav")), FileStatus::Missing);
    assert_eq!(file_status(Path::new("")), FileStatus::Missing);
    assert_ne!(file_status(Path::new("/tmp")), FileStatus::Ok);
    let nul = PathBuf::from(std::ffi::OsString::from("bad\0name.wav"));
    assert_eq!(file_status(&nul), FileStatus::Error);
    let long = PathBuf::from(format!("/{}", "x".repeat(5000)));
    assert_ne!(file_status(&long), FileStatus::Ok);
}

#[test]
fn opening_a_damaged_database_is_an_error_not_a_panic_or_silent_reset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.db");
    {
        let library = LocalLibrary::open(&path).unwrap();
        support::populate_library(&library, &dir.path().join("music"), 3, 0.2);
    }
    let good = std::fs::read(dir.path().join("library.db")).unwrap();
    let garbage = dir.path().join("garbage.db");
    std::fs::write(&garbage, b"this is not a sqlite database at all, just text").unwrap();
    assert!(LocalLibrary::open(&garbage).is_err());
    assert_eq!(std::fs::read(&garbage).unwrap(), b"this is not a sqlite database at all, just text", "a damaged file is left untouched");
    let cut = dir.path().join("cut.db");
    std::fs::write(&cut, &good[..good.len() / 2]).unwrap();
    let outcome = std::panic::catch_unwind(|| LocalLibrary::open(&cut).and_then(|l| l.get_all_tracks()));
    assert!(outcome.is_ok(), "a half database must not panic");
    let empty = dir.path().join("empty.db");
    std::fs::write(&empty, b"").unwrap();
    LocalLibrary::open(&empty).expect("an empty file is a fresh library");
}

#[test]
#[ignore = "export bug for the writer owner: two tracks with the same artist and file name share one USB path, see docs/architecture/robustness-2026-11.md"]
fn export_keeps_every_track_when_artist_and_file_name_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    let music = dir.path().join("music");
    support::write_wav(&music.join("one").join("song.wav"), 220.0, 0.3);
    support::write_wav(&music.join("two").join("song.wav"), 330.0, 0.3);
    support::write_wav(&music.join("three").join("\u{1F3A7} \u{5E9}\u{5DC}\u{5D5}\u{5DD}.wav"), 440.0, 0.3);
    let mut tracks = pioneer_usb_writer::scanner::scan_directory(&music).unwrap();
    for t in &mut tracks {
        t.artist = "Same Artist".into();
        t.usb_path = format!("/Contents/Same Artist/{}", t.source_path.file_name().unwrap().to_string_lossy());
    }
    tracks[0].title = "\u{1F3A7}".repeat(3000);
    tracks[1].title = "\u{202E}evil\nnew line".into();
    let ids = library.add_tracks(&tracks).unwrap();
    for id in &ids {
        library.set_analysis(*id, &support::make_analysis()).unwrap();
    }
    let out = dir.path().join("usb");
    std::fs::create_dir_all(&out).unwrap();
    library.write_usb(&out).expect("export finishes");
    let audio: Vec<PathBuf> = walkdir_files(&out).into_iter().filter(|p| p.starts_with(out.join("Contents"))).collect();
    assert_eq!(audio.len(), 3, "every track needs its own audio file on the stick: {audio:?}");
}

fn walkdir_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

fn library_with_tracks(dir: &Path) -> PathBuf {
    let path = dir.join("library.db");
    let library = LocalLibrary::open(&path).unwrap();
    support::populate_library(&library, &dir.join("music"), 3, 0.2);
    path
}

fn raw(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

fn track_count(path: &Path) -> i64 {
    raw(path).query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0)).unwrap()
}

fn version_rows(path: &Path) -> Vec<String> {
    let conn = raw(path);
    let mut stmt = conn.prepare("SELECT CAST(version AS TEXT) FROM schema_version").unwrap();
    stmt.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

#[test]
fn a_library_without_its_version_row_is_an_error_not_a_fresh_init() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    raw(&path).execute("DELETE FROM schema_version", []).unwrap();
    let err = LocalLibrary::open(&path).err().expect("missing version row must not open");
    assert!(err.to_string().contains("version"), "{err:#}");
    assert!(version_rows(&path).is_empty(), "the library must not be stamped as current");
    assert_eq!(track_count(&path), 3);
}

#[test]
fn a_library_without_its_version_table_is_an_error_not_a_fresh_init() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    raw(&path).execute("DROP TABLE schema_version", []).unwrap();
    assert!(LocalLibrary::open(&path).is_err());
    let has_table: bool = raw(&path)
        .query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE name = 'schema_version'", [], |r| r.get(0))
        .unwrap();
    assert!(!has_table, "no schema created over existing tables");
    assert_eq!(track_count(&path), 3);
}

#[test]
fn an_unreadable_version_is_an_error_not_a_fresh_init() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    raw(&path).execute("UPDATE schema_version SET version = 'garbage'", []).unwrap();
    assert!(LocalLibrary::open(&path).is_err());
    assert_eq!(version_rows(&path), vec!["garbage".to_string()]);
    assert_eq!(track_count(&path), 3);
}

#[test]
fn a_library_from_a_newer_app_is_a_clear_error_and_not_downgraded() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    raw(&path).execute("UPDATE schema_version SET version = 99", []).unwrap();
    let err = LocalLibrary::open(&path).err().expect("newer version must not open");
    assert!(err.to_string().contains("newer"), "{err:#}");
    assert_eq!(version_rows(&path), vec!["99".to_string()]);
    assert_eq!(track_count(&path), 3);
}

#[test]
fn a_damaged_header_is_an_error_and_the_file_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    raw(&path).query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[..16].copy_from_slice(b"not sqlite head!");
    std::fs::write(&path, &bytes).unwrap();
    assert!(LocalLibrary::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_locked_library_is_an_error_and_is_never_initialised() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_tracks(dir.path());
    let current = version_rows(&path);
    let holder = raw(&path);
    holder
        .execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE; DELETE FROM schema_version;")
        .unwrap();
    assert!(LocalLibrary::open(&path).is_err(), "a locked library must not open");
    holder.execute_batch("ROLLBACK").unwrap();
    drop(holder);
    assert_eq!(version_rows(&path), current);
    assert_eq!(track_count(&path), 3);
    LocalLibrary::open(&path).expect("opens once the lock is gone");
}

#[test]
fn a_database_file_with_no_tables_is_a_fresh_library() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.db");
    raw(&path).execute_batch("PRAGMA user_version = 0; VACUUM;").unwrap();
    let library = LocalLibrary::open(&path).unwrap();
    assert!(library.get_all_tracks().unwrap().is_empty());
    assert_eq!(version_rows(&path).len(), 1);
}
