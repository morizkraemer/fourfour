use std::path::{Path, PathBuf};

use pioneer_library::LocalLibrary;
use pioneer_library::rekordbox::{self, ImportOptions, SourceKind};
use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, Track, WaveformPreview,
};
use pioneer_usb_writer::writer;
use rusqlite::{Connection, params};
use tempfile::TempDir;

const MASTER_DB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

fn write_wav(path: &Path) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn grid(tempo: u32) -> BeatGrid {
    BeatGrid {
        beats: (0..8u32)
            .map(|i| Beat { bar_position: (i % 4 + 1) as u8, time_ms: 100 + i * 500, tempo })
            .collect(),
    }
}

fn analysis(tempo: u32, cues: Vec<CuePoint>) -> AnalysisResult {
    analysis_with_key(tempo, "", cues)
}

fn analysis_with_key(tempo: u32, key: &str, cues: Vec<CuePoint>) -> AnalysisResult {
    let mut waveform = [0u8; 400];
    for (i, b) in waveform.iter_mut().enumerate() {
        *b = (i % 32) as u8;
    }
    AnalysisResult {
        beat_grid: grid(tempo),
        waveform: WaveformPreview { data: waveform },
        bpm: tempo as f64 / 100.0,
        key: key.to_string(),
        energy: None,
        cue_points: cues,
        color_waveform: None,
    }
}

fn writer_track(path: &Path, usb_path: &str) -> Track {
    Track {
        id: 1,
        source_path: path.to_path_buf(),
        usb_path: usb_path.to_string(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 0,
        disc_number: 0,
        track_number: 0,
        tempo: 12800,
        key: String::new(),
        duration_secs: 2.0,
        sample_rate: 8000,
        bitrate: 128,
        file_size: 0,
        artwork: None,
    }
}

struct MasterDbFixture {
    _root: TempDir,
    db: PathBuf,
    share: PathBuf,
    audio: PathBuf,
}

fn master_db_fixture() -> MasterDbFixture {
    let root = tempfile::tempdir().unwrap();
    let audio = root.path().join("audio");
    let share = root.path().join("share");
    write_wav(&audio.join("House/ALPHA.wav"));
    write_wav(&audio.join("Techno/Beta.wav"));

    let anlz_rel = "/PIONEER/USBANLZ/abc/ANLZ0000.DAT";
    let anlz_dat = share.join(anlz_rel.trim_start_matches('/'));
    std::fs::create_dir_all(anlz_dat.parent().unwrap()).unwrap();
    let t = writer_track(&audio.join("House/ALPHA.wav"), "/Contents/Alpha.wav");
    let a = analysis(12800, Vec::new());
    writer::anlz::write_anlz_dat(&anlz_dat, &t, &a).unwrap();
    writer::anlz::write_anlz_ext(&anlz_dat.with_extension("EXT"), &t, &a).unwrap();

    let db = root.path().join("master.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(&format!("PRAGMA key = '{MASTER_DB_KEY}';")).unwrap();
    conn.execute_batch(
        "CREATE TABLE djmdArtist (ID VARCHAR PRIMARY KEY, Name VARCHAR);
         CREATE TABLE djmdAlbum (ID VARCHAR PRIMARY KEY, Name VARCHAR);
         CREATE TABLE djmdGenre (ID VARCHAR PRIMARY KEY, Name VARCHAR);
         CREATE TABLE djmdLabel (ID VARCHAR PRIMARY KEY, Name VARCHAR);
         CREATE TABLE djmdKey (ID VARCHAR PRIMARY KEY, ScaleName VARCHAR);
         CREATE TABLE djmdColor (ID VARCHAR PRIMARY KEY, Commnt VARCHAR);
         CREATE TABLE djmdContent (ID VARCHAR PRIMARY KEY, FolderPath VARCHAR, Title VARCHAR,
            ArtistID VARCHAR, AlbumID VARCHAR, GenreID VARCHAR, LabelID VARCHAR, RemixerID VARCHAR,
            KeyID VARCHAR, Commnt TEXT, ReleaseYear INTEGER, DiscNo INTEGER, TrackNo INTEGER,
            BPM INTEGER, Length INTEGER, SampleRate INTEGER, BitRate INTEGER, FileSize INTEGER,
            ImagePath VARCHAR, AnalysisDataPath VARCHAR, Rating INTEGER, ColorID VARCHAR,
            rb_local_deleted TINYINT DEFAULT 0);
         CREATE TABLE djmdCue (ID VARCHAR PRIMARY KEY, ContentID VARCHAR, InMsec INTEGER,
            OutMsec INTEGER, Kind INTEGER, ColorTableIndex INTEGER, ActiveLoop INTEGER,
            Comment VARCHAR, rb_local_deleted TINYINT DEFAULT 0);
         CREATE TABLE djmdPlaylist (ID VARCHAR PRIMARY KEY, Seq INTEGER, Name VARCHAR,
            Attribute INTEGER, ParentID VARCHAR, rb_local_deleted TINYINT DEFAULT 0);
         CREATE TABLE djmdSongPlaylist (ID VARCHAR PRIMARY KEY, PlaylistID VARCHAR, ContentID VARCHAR,
            TrackNo INTEGER, rb_local_deleted TINYINT DEFAULT 0);
         CREATE TABLE djmdProperty (DBID VARCHAR PRIMARY KEY, BaseDBDrive VARCHAR, CurrentDBDrive VARCHAR);
         INSERT INTO djmdArtist VALUES ('a1', 'Artist One');
         INSERT INTO djmdGenre VALUES ('g1', 'House');
         INSERT INTO djmdKey VALUES ('k1', '8A');
         INSERT INTO djmdColor VALUES ('2', 'Red');
         INSERT INTO djmdProperty VALUES ('db', '', '');",
    )
    .unwrap();
    let insert_content = "INSERT INTO djmdContent (ID, FolderPath, Title, ArtistID, GenreID, KeyID, BPM,
            Length, FileSize, AnalysisDataPath, Rating, ColorID)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 2, ?8, ?9, ?10, ?11)";
    let alpha_size = std::fs::metadata(audio.join("House/ALPHA.wav")).unwrap().len();
    conn.execute(
        insert_content,
        params!["100", "/Users/dj/Music/House/Alpha.wav", "Alpha", "a1", "g1", "k1", 12800, alpha_size, anlz_rel, 3, "2"],
    )
    .unwrap();
    conn.execute(
        insert_content,
        params!["101", "/Users/dj/Music/Techno/Beta.wav", "Beta", "a1", "", "", 0, alpha_size, "", 0, "0"],
    )
    .unwrap();
    conn.execute(
        insert_content,
        params!["102", "/Users/dj/Music/Gone.wav", "Gone", "a1", "", "", 0, 1, "", 0, "0"],
    )
    .unwrap();
    for (id, in_ms, out_ms, kind, color, active_loop, name) in [
        ("c1", 1000, -1, 1, 5, 0, "Intro"),
        ("c2", 2000, -1, 0, 0, 0, ""),
        ("c3", 3000, 4000, 2, 7, 1, "Loop"),
    ] {
        conn.execute(
            "INSERT INTO djmdCue (ID, ContentID, InMsec, OutMsec, Kind, ColorTableIndex, ActiveLoop, Comment)
             VALUES (?1, '100', ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, in_ms, out_ms, kind, color, active_loop, name],
        )
        .unwrap();
    }
    conn.execute_batch(
        "INSERT INTO djmdPlaylist VALUES ('p1', 1, 'Sets', 1, 'root', 0);
         INSERT INTO djmdPlaylist VALUES ('p2', 1, 'Friday', 0, 'p1', 0);
         INSERT INTO djmdPlaylist VALUES ('p3', 2, 'Top', 0, 'root', 0);
         INSERT INTO djmdPlaylist VALUES ('p4', 3, 'Smart', 4, 'root', 0);
         INSERT INTO djmdSongPlaylist VALUES ('s1', 'p2', '100', 1, 0);
         INSERT INTO djmdSongPlaylist VALUES ('s2', 'p2', '102', 2, 0);
         INSERT INTO djmdSongPlaylist VALUES ('s3', 'p2', '101', 3, 0);
         INSERT INTO djmdSongPlaylist VALUES ('s4', 'p3', '101', 1, 0);",
    )
    .unwrap();
    drop(conn);

    MasterDbFixture { _root: root, db, share, audio }
}

fn options(fixture: &MasterDbFixture) -> ImportOptions {
    ImportOptions {
        remap_from: None,
        remap_to: Some(fixture.audio.to_string_lossy().to_string()),
        include_missing: false,
    }
}

fn file_library() -> (TempDir, LocalLibrary) {
    let dir = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    (dir, library)
}

fn no_progress(_: rekordbox::ImportProgress) {}

#[test]
fn masterdb_preview_reports_counts_remap_and_missing_files() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();

    let preview = library.rekordbox_preview(&source, &options(&fx)).unwrap();

    assert_eq!(preview.source, SourceKind::MasterDb);
    assert_eq!(preview.tracks, 3);
    assert_eq!(preview.files_found, 2);
    assert_eq!(preview.files_missing, 1);
    assert_eq!(preview.missing_examples.len(), 1);
    assert_eq!(preview.detected_root, "/Users/dj/Music");
    assert_eq!(preview.remap.as_ref().unwrap().from_prefix, "/Users/dj/Music");
    assert_eq!((preview.playlists, preview.folders, preview.skipped_smart_playlists), (2, 1, 1));
    assert_eq!((preview.hot_cues, preview.memory_cues), (2, 1));
    assert_eq!(preview.tracks_with_grid, 1);
    assert_eq!((preview.conflicts, preview.new_tracks), (0, 3));
}

#[test]
fn masterdb_import_brings_tags_grid_cues_playlists_and_marks_them_imported() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();

    let report = library
        .import_rekordbox_from(&source, &options(&fx), &no_progress)
        .unwrap();

    assert_eq!(report.tracks_imported, 2);
    assert_eq!(report.tracks_skipped_missing, 1);
    assert_eq!((report.files_found, report.files_missing), (2, 1));
    assert_eq!((report.grids_imported, report.hot_cues_imported, report.memory_cues_imported), (1, 2, 1));
    assert_eq!(report.playlists_imported, 2);

    let tracks = library.get_all_tracks().unwrap();
    let alpha = tracks.iter().find(|t| t.title == "Alpha").unwrap();
    assert_eq!(alpha.artist, "Artist One");
    assert_eq!(alpha.genre, "House");
    assert_eq!(alpha.key, "8A");
    assert_eq!(alpha.tempo, 12800);
    assert_eq!(alpha.source_path, fx.audio.join("House/ALPHA.wav"));
    assert_eq!(alpha.usb_path, "/Contents/Artist One/ALPHA.wav");

    let id = alpha.id as i64;
    let stored = library.get_analysis(id).unwrap().unwrap();
    assert_eq!(stored.beat_grid.beats.len(), 8);
    assert_eq!(stored.bpm, 128.0);
    assert_eq!(stored.key, "8A");
    let cue_times: Vec<(u32, u32, Option<u32>)> = stored
        .cue_points
        .iter()
        .map(|c| (c.hot_cue_number, c.time_ms, c.loop_time_ms))
        .collect();
    assert!(cue_times.contains(&(1, 1000, None)));
    assert!(cue_times.contains(&(0, 2000, None)));
    assert!(cue_times.contains(&(2, 3000, Some(4000))));

    let meta = library.imported_meta(id).unwrap();
    assert_eq!((meta.rating, meta.color.as_deref()), (Some(3), Some("Red")));
    assert!(meta.grid_locked && meta.cues_locked);
    let intro = meta.cues.iter().find(|c| c.name == "Intro").unwrap();
    assert_eq!(intro.color.as_deref(), Some("table:5"));

    let beta = tracks.iter().find(|t| t.title == "Beta").unwrap();
    assert!(library.get_analysis(beta.id as i64).unwrap().is_none());

    let playlists = library.get_all_playlists().unwrap();
    let mut names: Vec<_> = playlists.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Sets / Friday", "Top"]);
    let friday = playlists.iter().find(|p| p.name == "Sets / Friday").unwrap();
    assert_eq!(friday.track_ids.len(), 2);
    assert_eq!(friday.track_ids[0], alpha.id);
}

#[test]
fn imported_grid_and_cues_survive_reanalysis_until_released() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();
    library.import_rekordbox_from(&source, &options(&fx), &no_progress).unwrap();
    let id = library
        .get_all_tracks()
        .unwrap()
        .iter()
        .find(|t| t.title == "Alpha")
        .unwrap()
        .id as i64;

    let mut fresh = analysis(9000, Vec::new());
    fresh.beat_grid.beats.truncate(2);
    library.set_analysis(id, &fresh).unwrap();

    let kept = library.get_analysis(id).unwrap().unwrap();
    assert_eq!(kept.beat_grid.beats.len(), 8);
    assert_eq!(kept.bpm, 128.0);
    assert_eq!(kept.cue_points.len(), 3);

    library.release_imported_locks(id).unwrap();
    library.set_analysis(id, &fresh).unwrap();
    let replaced = library.get_analysis(id).unwrap().unwrap();
    assert_eq!(replaced.beat_grid.beats.len(), 2);
    assert!(replaced.cue_points.is_empty());
}

#[test]
fn existing_tracks_keep_user_edits_get_empty_fields_filled_and_nothing_is_deleted() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();

    let mut mine = writer_track(&fx.audio.join("house/alpha.WAV"), "/Contents/Mine/alpha.wav");
    mine.title = "My Edit".into();
    mine.id = 0;
    mine.tempo = 0;
    let existing_id = library.add_track(&mine).unwrap();
    let user_analysis = analysis(14000, Vec::new());
    library.set_analysis(existing_id, &user_analysis).unwrap();
    let untouched_id = library
        .add_track(&writer_track(&fx.audio.join("elsewhere.wav"), "/Contents/x/elsewhere.wav"))
        .unwrap();
    let pl = library.create_playlist("Top").unwrap();
    library.set_playlist_tracks(pl, &[untouched_id]).unwrap();

    let preview = library.rekordbox_preview(&source, &options(&fx)).unwrap();
    assert_eq!(preview.conflicts, 1);
    assert_eq!(preview.playlist_conflicts, 1);

    let report = library.import_rekordbox_from(&source, &options(&fx), &no_progress).unwrap();
    assert_eq!((report.tracks_imported, report.tracks_merged), (1, 1));
    assert_eq!((report.playlists_imported, report.playlists_merged), (1, 1));

    let merged = library.get_track(existing_id).unwrap().unwrap();
    assert_eq!(merged.title, "My Edit");
    assert_eq!(merged.usb_path, "/Contents/Mine/alpha.wav");
    assert_eq!(merged.genre, "House");
    assert_eq!(merged.tempo, 12800);
    assert_eq!(library.get_analysis(existing_id).unwrap().unwrap().bpm, 140.0);
    assert!(library.get_track(untouched_id).unwrap().is_some());

    let top = library.get_all_playlists().unwrap().into_iter().find(|p| p.name == "Top").unwrap();
    assert!(top.track_ids.contains(&(untouched_id as u32)));
    assert_eq!(top.track_ids.len(), 2);

    let before = library.get_all_tracks().unwrap().len();
    let again = library.import_rekordbox_from(&source, &options(&fx), &no_progress).unwrap();
    assert_eq!(again.tracks_imported, 0);
    assert_eq!(library.get_all_tracks().unwrap().len(), before);
}

#[test]
fn include_missing_keeps_tracks_whose_files_are_gone() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();
    let mut opts = options(&fx);
    opts.include_missing = true;

    let report = library.import_rekordbox_from(&source, &opts, &no_progress).unwrap();

    assert_eq!((report.tracks_imported, report.tracks_skipped_missing), (3, 0));
    let friday = library
        .get_all_playlists()
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Sets / Friday")
        .unwrap();
    assert_eq!(friday.track_ids.len(), 3);
}

#[test]
fn caller_can_name_the_prefix_to_replace() {
    let fx = master_db_fixture();
    let source = rekordbox::load_masterdb(&fx.db, Some(&fx.share)).unwrap();
    let (_dir, library) = file_library();
    let mut opts = options(&fx);
    opts.remap_from = Some("/users/DJ/music/house".into());
    opts.remap_to = Some(fx.audio.join("House").to_string_lossy().to_string());

    let preview = library.rekordbox_preview(&source, &opts).unwrap();

    assert_eq!(preview.files_found, 1);
    assert_eq!(preview.remap.unwrap().from_prefix, "/users/DJ/music/house");
}

#[test]
fn non_master_db_is_rejected_with_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let bogus = dir.path().join("master.db");
    std::fs::write(&bogus, b"not a database at all, just bytes").unwrap();
    let err = rekordbox::load_masterdb(&bogus, None).unwrap_err();
    assert!(format!("{err:#}").contains("master.db"));
}

#[test]
fn usb_stick_written_by_the_writer_reads_back_through_the_importer() {
    let src = tempfile::tempdir().unwrap();
    let usb = tempfile::tempdir().unwrap();
    let mut tracks = Vec::new();
    for (i, (file, title)) in [("one.wav", "One"), ("two.wav", "Two")].into_iter().enumerate() {
        let path = src.path().join(file);
        write_wav(&path);
        let mut t = writer_track(&path, &format!("/Contents/Artist/{file}"));
        t.id = i as u32 + 1;
        t.title = title.into();
        t.artist = "Artist".into();
        t.key = "5A".into();
        t.file_size = std::fs::metadata(&path).unwrap().len();
        tracks.push(t);
    }
    let analyses = vec![
        analysis_with_key(12800, "5A", vec![
            CuePoint { hot_cue_number: 0, time_ms: 500, loop_time_ms: None, ..Default::default() },
            CuePoint { hot_cue_number: 1, time_ms: 1000, loop_time_ms: None, ..Default::default() },
            CuePoint { hot_cue_number: 2, time_ms: 1500, loop_time_ms: Some(1900), ..Default::default() },
        ]),
        analysis(12400, Vec::new()),
    ];
    let playlists = vec![Playlist { id: 1, name: "Warmup".into(), track_ids: vec![2, 1] }];
    writer::filesystem::write_usb(usb.path(), &tracks, &analyses, &playlists).unwrap();

    let source = rekordbox::load_usb(usb.path()).unwrap();
    let (_dir, library) = file_library();
    let opts = ImportOptions::default();

    let preview = library.rekordbox_preview(&source, &opts).unwrap();
    assert_eq!(preview.source, SourceKind::Usb);
    assert_eq!((preview.tracks, preview.files_found, preview.files_missing), (2, 2, 0));
    assert_eq!(preview.playlists, 1);

    let report = library.import_rekordbox_from(&source, &opts, &no_progress).unwrap();
    assert_eq!((report.tracks_imported, report.playlists_imported), (2, 1));
    assert_eq!(report.grids_imported, 2);
    assert_eq!((report.hot_cues_imported, report.memory_cues_imported), (2, 1));

    let one = library.get_all_tracks().unwrap().into_iter().find(|t| t.title == "One").unwrap();
    assert_eq!(one.key, "5A");
    assert_eq!(one.source_path, usb.path().join("Contents/Artist/one.wav"));
    let stored = library.get_analysis(one.id as i64).unwrap().unwrap();
    assert_eq!(stored.beat_grid.beats.len(), 8);
    assert!(stored.cue_points.iter().any(|c| c.hot_cue_number == 2 && c.loop_time_ms.is_some()));

    let warmup = library.get_all_playlists().unwrap().remove(0);
    assert_eq!(warmup.name, "Warmup");
    assert_eq!(warmup.track_ids.len(), 2);
    assert_ne!(warmup.track_ids[0], one.id);
}

#[test]
fn xml_export_imports_and_keeps_grid_and_cues_for_later_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("audio");
    write_wav(&audio.join("A.wav"));
    let xml = format!(
        r#"<?xml version="1.0"?>
<DJ_PLAYLISTS Version="1.0.0"><COLLECTION Entries="1">
<TRACK TrackID="7" Name="Xml Track" Artist="X" TotalTime="4" AverageBpm="120.00" Rating="153" Tonality="3A" Location="file://localhost{}/A.wav">
<TEMPO Inizio="0.000" Bpm="120.00" Metro="4/4" Battito="1"/>
<POSITION_MARK Name="Hook" Type="0" Start="1.000" Num="0" Red="255" Green="0" Blue="0"/>
</TRACK></COLLECTION>
<PLAYLISTS><NODE Type="0" Name="ROOT" Count="1"><NODE Name="Set" Type="1" KeyType="0" Entries="1"><TRACK Key="7"/></NODE></NODE></PLAYLISTS>
</DJ_PLAYLISTS>"#,
        audio.display()
    );
    let xml_path = dir.path().join("rekordbox.xml");
    std::fs::write(&xml_path, xml).unwrap();

    let source = rekordbox::load_xml(&xml_path).unwrap();
    let (_lib_dir, library) = file_library();
    let report = library
        .import_rekordbox_from(&source, &ImportOptions::default(), &no_progress)
        .unwrap();

    assert_eq!((report.tracks_imported, report.playlists_imported), (1, 1));
    assert_eq!((report.grids_imported, report.hot_cues_imported, report.cues_without_analysis), (1, 1, 1));
    let track = library.get_all_tracks().unwrap().remove(0);
    assert_eq!((track.title.as_str(), track.tempo, track.key.as_str()), ("Xml Track", 12000, "3A"));
    let id = track.id as i64;
    let meta = library.imported_meta(id).unwrap();
    assert_eq!(meta.rating, Some(3));
    assert_eq!(meta.cues[0].color.as_deref(), Some("#ff0000"));
    assert!(library.get_analysis(id).unwrap().is_none());

    let mut later = analysis(9000, Vec::new());
    later.beat_grid.beats.truncate(1);
    library.set_analysis(id, &later).unwrap();
    let stored = library.get_analysis(id).unwrap().unwrap();
    assert_eq!(stored.bpm, 120.0);
    assert!(stored.beat_grid.beats.len() > 4);
    assert_eq!(stored.cue_points.len(), 1);
}
