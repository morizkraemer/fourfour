use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, SyncReport, Track, WaveformPreview};
use pioneer_usb_writer::stick::device_library::{self, table};
use pioneer_usb_writer::stick::dump::normalised_row;
use pioneer_usb_writer::stick::emit::write_usb_v2;
use pioneer_usb_writer::stick::plan::{apply, plan, Ownership, StickPlan, TrackAction, Wanted};
use pioneer_usb_writer::stick::{one_library, Stick, StickDevice, StickModel};
use tempfile::TempDir;

fn write_wav(path: &Path, seconds: u32) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * seconds * 2;
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
    std::fs::write(path, bytes).unwrap();
}

fn cover_png(shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(32, 32, image::Rgb([shade, 30, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn track(source_dir: &Path, id: u32, file: &str, title: &str, artist: &str) -> Track {
    let source_path = source_dir.join(file);
    write_wav(&source_path, 2);
    Track {
        usb_path: format!("/Contents/{artist}/{file}"),
        source_path: source_path.clone(),
        title: title.into(),
        artist: artist.into(),
        album: format!("{artist} LP"),
        genre: "House".into(),
        label: "Test Label".into(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: id,
        tempo: 12800,
        key: "8A".into(),
        duration_secs: 2.0,
        sample_rate: 8000,
        bitrate: 128,
        file_size: std::fs::metadata(&source_path).unwrap().len(),
        id,
        artwork: None,
    }
}

fn analysis(cues: Vec<CuePoint>) -> AnalysisResult {
    let beats = (0..8).map(|i| Beat { bar_position: (i % 4) as u8 + 1, time_ms: i * 468, tempo: 12800 }).collect();
    AnalysisResult {
        beat_grid: BeatGrid { beats },
        waveform: WaveformPreview { data: [0xA4; 400] },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: cues,
        color_waveform: None,
    }
}

fn cue(hot_cue_number: u32, time_ms: u32) -> CuePoint {
    CuePoint { hot_cue_number, time_ms, loop_time_ms: None, ..Default::default() }
}

struct Library {
    _source: TempDir,
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
}

impl Library {
    fn new(tracks: &[(u32, &str, &str, &str)]) -> Self {
        let source = TempDir::new().unwrap();
        let tracks: Vec<Track> =
            tracks.iter().map(|&(id, file, title, artist)| track(source.path(), id, file, title, artist)).collect();
        let analyses = tracks.iter().map(|_| analysis(Vec::new())).collect();
        Self { _source: source, tracks, analyses, playlists: Vec::new() }
    }

    fn with_playlist(mut self, id: u32, name: &str, track_ids: &[u32]) -> Self {
        self.playlists.push(Playlist { id, name: name.into(), track_ids: track_ids.to_vec(), ..Default::default() });
        self
    }

    fn wanted(&self) -> Wanted<'_> {
        Wanted { tracks: &self.tracks, analyses: &self.analyses, playlists: &self.playlists }
    }
}

fn ours() -> Library {
    Library::new(&[(10, "one.wav", "First Light", "Alpha"), (20, "two.wav", "Second Wind", "Beta")])
        .with_playlist(5, "Warmup", &[20, 10])
}

fn device() -> StickDevice {
    StickDevice { name: "FFTEST".into(), created_date: "2026-11-22".into(), ..StickDevice::default() }
}

fn planned(usb: &Path, library: &Library, owned: &Ownership) -> StickPlan {
    plan(&Stick::read(usb).unwrap(), library.wanted(), owned, device()).unwrap()
}

fn synced(usb: &Path, library: &Library, owned: &Ownership) -> (SyncReport, Ownership) {
    let plan = planned(usb, library, owned);
    let report = apply(usb, &plan, library.wanted()).unwrap();
    (report, plan.ownership)
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

fn rekordbox_like_stick() -> (Library, TempDir, StickModel) {
    let theirs = Library::new(&[
        (1, "r1.wav", "Their One", "Gamma"),
        (2, "r2.wav", "Their Two", "Delta"),
        (3, "r3.wav", "Their Three", "Gamma"),
    ])
    .with_playlist(1, "Their Set", &[3, 1, 2]);
    let usb = TempDir::new().unwrap();
    let model = write_usb_v2(usb.path(), &theirs.tracks, &theirs.analyses, &theirs.playlists, device()).unwrap();
    let emptied = StickModel { tracks: Vec::new(), playlists: Vec::new(), artists: Vec::new(), albums: Vec::new(), ..model.clone() };
    one_library::write(&usb.path().join("PIONEER/rekordbox/exportLibrary.db"), &emptied).unwrap();
    (theirs, usb, model)
}

fn raw_rows_by_id(usb: &Path, kind: u32, id_at: usize) -> BTreeMap<u32, Vec<u8>> {
    let bytes = std::fs::read(usb.join("PIONEER/rekordbox/export.pdb")).unwrap();
    device_library::table_rows(&bytes)
        .unwrap()
        .into_iter()
        .find(|rows| rows.table == kind)
        .unwrap()
        .rows
        .iter()
        .map(|row| (u32::from_le_bytes(row[id_at..id_at + 4].try_into().unwrap()), normalised_row(kind, row)))
        .collect()
}

#[test]
fn a_blank_stick_gets_everything_and_a_second_sync_writes_nothing() {
    let library = ours();
    let usb = TempDir::new().unwrap();
    let (report, owned) = synced(usb.path(), &library, &Ownership::default());
    assert_eq!(counts(&report), (2, 0, 0, 0, 0));
    assert_eq!(owned.paths.len(), 2);
    assert_eq!(owned.playlist_ids.len(), 1);

    let stick = Stick::read(usb.path()).unwrap();
    for model in [stick.device_library.as_ref().unwrap(), stick.one_library.as_ref().unwrap()] {
        assert_eq!(model.tracks.len(), 2);
        assert_eq!(model.playlist_tracks(model.playlists[0].id).iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Second Wind", "First Light"]);
        assert_eq!(model.genre_name(model.tracks[0].genre_id), "House");
        assert_eq!(model.key_name(model.tracks[0].key_id), "Db");
    }
    for track in &stick.library().unwrap().tracks {
        assert!(stick.audio_path(track).is_file());
        assert!(stick.read_analysis(track).unwrap().dat.is_some());
    }

    let before = listing(usb.path());
    let again = planned(usb.path(), &library, &owned);
    assert!(again.is_noop());
    assert_eq!(counts(&apply(usb.path(), &again, library.wanted()).unwrap()), (0, 0, 0, 0, 2));
    assert_eq!(listing(usb.path()), before);
    assert_eq!(again.ownership, owned);
}

#[test]
fn foreign_tracks_and_playlists_stay_in_both_databases() {
    let (_theirs, usb, their_model) = rekordbox_like_stick();
    let their_rows = raw_rows_by_id(usb.path(), table::TRACKS, 0x48);
    let their_artists = raw_rows_by_id(usb.path(), table::ARTISTS, 4);
    let before = listing(usb.path());
    assert_eq!(Stick::read(usb.path()).unwrap().one_library.unwrap().tracks.len(), 0);

    let library = ours();
    let plan = planned(usb.path(), &library, &Ownership::default());
    assert_eq!(plan.foreign_track_count, 3);
    assert_eq!(counts(&apply(usb.path(), &plan, library.wanted()).unwrap()), (2, 0, 0, 0, 0));

    let stick = Stick::read(usb.path()).unwrap();
    for model in [stick.device_library.as_ref().unwrap(), stick.one_library.as_ref().unwrap()] {
        assert_eq!(model.tracks.len(), 5);
        let their_set = model.playlists.iter().find(|playlist| playlist.name == "Their Set").unwrap();
        assert_eq!(their_set.id, their_model.playlists[0].id);
        assert_eq!(their_set.track_ids, [3, 1, 2]);
        assert_eq!(model.track(1).unwrap().title, "Their One");
        assert_eq!(model.artist_name(model.track(2).unwrap().artist_id), "Delta");
        let warmup = model.playlists.iter().find(|playlist| playlist.name == "Warmup").unwrap();
        assert_eq!(model.playlist_tracks(warmup.id).iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Second Wind", "First Light"]);
        assert_eq!(model.children(0).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Their Set", "Warmup"]);
    }
    let pdb = stick.device_library.as_ref().unwrap();
    assert_eq!(pdb.track_by_path("/Contents/Alpha/one.wav").unwrap().id, 4);
    assert_eq!(pdb.track_by_path("/Contents/Beta/two.wav").unwrap().id, 5);
    assert_eq!(pdb.device.name, "FFTEST");

    let rows_now = raw_rows_by_id(usb.path(), table::TRACKS, 0x48);
    for (id, row) in &their_rows {
        assert_eq!(rows_now.get(id), Some(row), "foreign track row {id} changed");
    }
    let artists_now = raw_rows_by_id(usb.path(), table::ARTISTS, 4);
    for (id, row) in &their_artists {
        assert_eq!(artists_now.get(id), Some(row), "foreign artist row {id} changed");
    }
    let after = listing(usb.path());
    for (file, stamp) in before.iter().filter(|(file, _)| !file.starts_with("PIONEER/rekordbox/")) {
        assert_eq!(after.get(file), Some(stamp), "foreign file {file} was touched");
    }
}

#[test]
fn removing_our_tracks_leaves_foreign_files_and_rows() {
    let (_theirs, usb, _) = rekordbox_like_stick();
    let library = ours();
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());

    let fewer = Library { tracks: vec![library.tracks[1].clone()], analyses: vec![library.analyses[1].clone()], playlists: vec![Playlist { id: 5, name: "Warmup".into(), track_ids: vec![20], ..Default::default() }], _source: TempDir::new().unwrap() };
    let warmup_before = Stick::read(usb.path()).unwrap().library().unwrap().playlists.iter().find(|p| p.name == "Warmup").unwrap().id;
    let (report, owned) = synced(usb.path(), &fewer, &owned);
    assert_eq!(counts(&report), (0, 0, 0, 1, 1));

    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    assert_eq!(model.tracks.len(), 4);
    assert!(model.track_by_path("/Contents/Alpha/one.wav").is_none());
    assert!(!usb.path().join("Contents/Alpha/one.wav").exists());
    assert!(!usb.path().join("Contents/Alpha").exists());
    assert!(model.artists.iter().all(|artist| artist.name != "Alpha"));
    assert_eq!(model.track_by_path("/Contents/Beta/two.wav").unwrap().id, 5);
    let warmup = model.playlists.iter().find(|p| p.name == "Warmup").unwrap();
    assert_eq!((warmup.id, warmup.track_ids.clone()), (warmup_before, vec![5]));
    for their in ["r1.wav", "r2.wav", "r3.wav"] {
        assert!(model.tracks.iter().any(|track| track.filename == their));
    }
    assert!(usb.path().join("Contents/Gamma/r1.wav").is_file());

    let nothing = Library::new(&[]);
    let (report, owned) = synced(usb.path(), &nothing, &owned);
    assert_eq!(counts(&report), (0, 0, 0, 1, 0));
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.library().unwrap().tracks.len(), 3);
    assert_eq!(stick.one_library.as_ref().unwrap().tracks.len(), 3);
    assert_eq!(stick.library().unwrap().playlists.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Their Set"]);
    assert!(owned.paths.is_empty() && owned.playlist_ids.is_empty());
    for track in &stick.library().unwrap().tracks {
        assert!(stick.audio_path(track).is_file());
        assert!(stick.read_analysis(track).unwrap().dat.is_some());
    }
}

#[test]
fn a_new_track_never_reuses_the_id_of_a_removed_one_and_others_keep_theirs() {
    let usb = TempDir::new().unwrap();
    let first = Library::new(&[(1, "a.wav", "A", "X"), (2, "b.wav", "B", "X"), (3, "c.wav", "C", "X")]);
    let (_, owned) = synced(usb.path(), &first, &Ownership::default());
    let mut second = Library::new(&[(7, "c.wav", "C", "X"), (8, "d.wav", "D", "X"), (9, "a.wav", "A", "X")]);
    second.tracks[0].track_number = 3;
    second.tracks[2].track_number = 1;
    let (report, _) = synced(usb.path(), &second, &owned);
    assert_eq!(counts(&report), (1, 0, 0, 1, 2));
    let model = Stick::read(usb.path()).unwrap().library().unwrap().clone();
    let id_of = |file: &str| model.tracks.iter().find(|track| track.filename == file).unwrap().id;
    assert_eq!((id_of("a.wav"), id_of("c.wav"), id_of("d.wav")), (1, 3, 4));
    assert_eq!(model.tracks.iter().find(|track| track.filename == "a.wav").unwrap().date_added, "2026-11-22");
}

#[test]
fn artists_that_differ_only_in_case_stay_two_artists_and_do_not_update_forever() {
    let usb = TempDir::new().unwrap();
    let library = Library::new(&[(1, "a.wav", "A", "test artist"), (2, "b.wav", "B", "Test Artist")]);
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());
    let model = Stick::read(usb.path()).unwrap().library().unwrap().clone();
    assert_eq!(model.artists.iter().map(|artist| artist.name.as_str()).collect::<Vec<_>>(), ["test artist", "Test Artist"]);
    let again = planned(usb.path(), &library, &owned);
    assert!(again.is_noop(), "{:?}", again.tracks.iter().map(|track| track.action).collect::<Vec<_>>());
}

#[test]
fn a_changed_cue_rewrites_the_analysis_but_not_the_audio() {
    let usb = TempDir::new().unwrap();
    let mut library = ours();
    library.analyses[0].cue_points = vec![cue(1, 500)];
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());
    let stick = Stick::read(usb.path()).unwrap();
    let first = stick.library().unwrap().track_by_path("/Contents/Alpha/one.wav").unwrap().clone();
    let dat = usb.path().join(first.analyze_path.trim_start_matches('/'));
    let dat_before = std::fs::read(&dat).unwrap();
    let before = listing(usb.path());

    library.analyses[0].cue_points = vec![cue(1, 500), cue(2, 900)];
    let plan = planned(usb.path(), &library, &owned);
    assert_eq!(plan.tracks.iter().map(|track| track.action).collect::<Vec<_>>(), [TrackAction::Update, TrackAction::Unchanged]);
    apply(usb.path(), &plan, library.wanted()).unwrap();

    assert_ne!(std::fs::read(&dat).unwrap(), dat_before);
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.read_analysis(&first).unwrap().cues().len(), 2);
    assert_eq!(stick.one_library.as_ref().unwrap().track(first.id).unwrap().cues.len(), 2);
    let after = listing(usb.path());
    for audio in ["Contents/Alpha/one.wav", "Contents/Beta/two.wav"] {
        assert_eq!(after[audio], before[audio], "{audio} was copied again");
    }
    let other = stick.library().unwrap().track_by_path("/Contents/Beta/two.wav").unwrap();
    let other_dat = other.analyze_path.trim_start_matches('/').to_owned();
    assert_eq!(after[&other_dat], before[&other_dat]);
    assert!(planned(usb.path(), &library, &plan.ownership).is_noop());
}

#[test]
fn a_changed_title_or_a_changed_file_is_seen() {
    let usb = TempDir::new().unwrap();
    let mut library = ours();
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());

    library.tracks[0].title = "First Light (Dub)".into();
    let (report, owned) = synced(usb.path(), &library, &owned);
    assert_eq!(counts(&report), (0, 1, 0, 0, 1));
    assert_eq!(Stick::read(usb.path()).unwrap().library().unwrap().track(1).unwrap().title, "First Light (Dub)");

    write_wav(&library.tracks[1].source_path, 3);
    library.tracks[1].file_size = std::fs::metadata(&library.tracks[1].source_path).unwrap().len();
    let (report, _) = synced(usb.path(), &library, &owned);
    assert_eq!(counts(&report), (0, 0, 1, 0, 1));
    assert_eq!(std::fs::metadata(usb.path().join("Contents/Beta/two.wav")).unwrap().len(), library.tracks[1].file_size);
}

#[test]
fn trailing_dots_and_case_collisions_get_safe_paths() {
    let usb = TempDir::new().unwrap();
    let mut library = Library::new(&[(1, "a.wav", "A", "Artist Name."), (2, "b.wav", "B", "Mixed"), (3, "c.wav", "C", "Mixed")]);
    library.tracks[1].usb_path = "/Contents/Mixed/Same.wav".into();
    library.tracks[2].usb_path = "/Contents/mixed/SAME.wav".into();
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());

    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    let paths: Vec<&str> = model.tracks.iter().map(|track| track.path.as_str()).collect();
    assert_eq!(paths, ["/Contents/Artist Name/a.wav", "/Contents/Mixed/Same.wav", "/Contents/mixed/SAME (2).wav"]);
    assert_eq!(model.artist_name(model.tracks[0].artist_id), "Artist Name.");
    for track in &model.tracks {
        assert!(stick.audio_path(track).is_file(), "{}", track.path);
        let analysis = stick.read_analysis(track).unwrap();
        assert_eq!(analysis.dat.unwrap().path().as_deref(), Some(track.path.as_str()));
    }
    assert!(!usb.path().join("Contents/Artist Name.").exists());
    assert!(planned(usb.path(), &library, &owned).is_noop());
}

#[test]
fn a_wanted_track_that_is_already_there_as_a_foreign_file_is_left_alone() {
    let (theirs, usb, _) = rekordbox_like_stick();
    let mut library = ours();
    let mut same = theirs.tracks[0].clone();
    same.id = 30;
    same.title = "Our name for it".into();
    library.tracks.push(same);
    library.analyses.push(analysis(vec![cue(1, 100)]));
    library.playlists[0].track_ids = vec![30, 10];
    let before = listing(usb.path());

    let plan = planned(usb.path(), &library, &Ownership::default());
    assert_eq!(plan.tracks[2].action, TrackAction::AdoptedForeign);
    assert!(!plan.ownership.paths.contains("/Contents/Gamma/r1.wav"));
    apply(usb.path(), &plan, library.wanted()).unwrap();

    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    assert_eq!(model.tracks.len(), 5);
    assert_eq!(model.track(1).unwrap().title, "Their One");
    let warmup = model.playlists.iter().find(|p| p.name == "Warmup").unwrap();
    assert_eq!(warmup.track_ids, [1, 4]);
    let after = listing(usb.path());
    assert_eq!(after["Contents/Gamma/r1.wav"], before["Contents/Gamma/r1.wav"]);

    let (_, owned) = synced(usb.path(), &Library::new(&[]), &plan.ownership);
    assert!(usb.path().join("Contents/Gamma/r1.wav").is_file());
    assert!(owned.paths.is_empty());
}

#[test]
fn a_stick_whose_two_databases_disagree_is_refused() {
    let (_theirs, usb, model) = rekordbox_like_stick();
    let mut odd = model.clone();
    odd.tracks.truncate(1);
    odd.tracks[0].path = "/Contents/Elsewhere/only-here.wav".into();
    odd.playlists.clear();
    one_library::write(&usb.path().join("PIONEER/rekordbox/exportLibrary.db"), &odd).unwrap();
    let before = listing(usb.path());

    let library = ours();
    let error = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &Ownership::default(), device()).unwrap_err();
    assert!(format!("{error:#}").contains("disagree"), "{error:#}");
    assert_eq!(listing(usb.path()), before);
}

#[test]
fn syncing_nothing_onto_a_foreign_stick_writes_nothing() {
    let (_theirs, usb, _) = rekordbox_like_stick();
    let before = listing(usb.path());
    let nothing = Library::new(&[]);
    let plan = planned(usb.path(), &nothing, &Ownership::default());
    assert!(plan.is_noop());
    apply(usb.path(), &plan, nothing.wanted()).unwrap();
    assert_eq!(listing(usb.path()), before);
}

#[test]
fn artwork_ids_survive_and_obsolete_covers_are_removed() {
    let usb = TempDir::new().unwrap();
    let mut library = ours();
    library.tracks[0].artwork = Some(cover_png(200));
    library.tracks[1].artwork = Some(cover_png(20));
    let (_, owned) = synced(usb.path(), &library, &Ownership::default());
    let art = |name: &str| -> PathBuf { usb.path().join("PIONEER/Artwork/00001").join(name) };
    assert!(art("a1.jpg").is_file() && art("b2_m.jpg").is_file());
    assert!(planned(usb.path(), &library, &owned).is_noop());

    library.tracks[1].artwork = None;
    let (report, _) = synced(usb.path(), &library, &owned);
    assert_eq!(counts(&report), (0, 1, 0, 0, 1));
    assert!(art("a1.jpg").is_file());
    for gone in ["a2.jpg", "a2_m.jpg", "b2.jpg", "b2_m.jpg"] {
        assert!(!art(gone).exists(), "{gone}");
    }
    let model = Stick::read(usb.path()).unwrap().library().unwrap().clone();
    assert_eq!(model.artworks.len(), 1);
    assert_eq!((model.track(1).unwrap().artwork_id, model.track(2).unwrap().artwork_id), (1, 0));
}
