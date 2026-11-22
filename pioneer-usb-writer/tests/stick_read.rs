use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, Track, WaveformPreview};
use pioneer_usb_writer::stick::dump::{diff_dumps, dump_stick, DumpOptions};
use pioneer_usb_writer::stick::{device_library, one_library, LibraryKind, Stick, StickModel};
use pioneer_usb_writer::writer::filesystem::write_usb;
use tempfile::TempDir;

const RAW: DumpOptions = DumpOptions { raw: true, normalise: true, pages: true };

fn write_wav(path: &Path) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * 2 * 2;
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

fn track(source_dir: &Path, id: u32, file: &str, title: &str, artist: &str, key: &str, tempo: u32) -> Track {
    let source_path = source_dir.join(file);
    write_wav(&source_path);
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
        tempo,
        key: key.into(),
        duration_secs: 2.0,
        sample_rate: 8000,
        bitrate: 128,
        file_size: std::fs::metadata(&source_path).unwrap().len(),
        id,
        artwork: None,
    }
}

fn analysis(tempo: u32, key: &str, cues: Vec<CuePoint>) -> AnalysisResult {
    let beat_ms = 6_000_000 / tempo;
    let beats = (0..8).map(|i| Beat { bar_position: (i % 4) as u8 + 1, time_ms: i * beat_ms, tempo }).collect();
    AnalysisResult {
        beat_grid: BeatGrid { beats },
        waveform: WaveformPreview { data: [0xA4; 400] },
        bpm: tempo as f64 / 100.0,
        key: key.into(),
        energy: None,
        cue_points: cues,
        color_waveform: None,
    }
}

fn old_writer_stick() -> (TempDir, TempDir) {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let mut first = track(source.path(), 1, "one.wav", "First Light", "Alpha", "8A", 12800);
    first.comment = "c".repeat(140);
    let second = track(source.path(), 2, "two.wav", "Zweiter Wind \u{00fc} \u{65e5}\u{672c} \u{0416}", "Beta", "5B", 12400);
    let third = track(source.path(), 3, "three.wav", "Third Rail", "Alpha", "", 13000);
    let cues = vec![
        CuePoint { hot_cue_number: 0, time_ms: 500, loop_time_ms: None, ..Default::default() },
        CuePoint { hot_cue_number: 1, time_ms: 1000, loop_time_ms: None, name: "drop".into(), ..Default::default() },
        CuePoint { hot_cue_number: 2, time_ms: 1500, loop_time_ms: Some(1900), ..Default::default() },
    ];
    let analyses = vec![analysis(12800, "8A", cues), analysis(12400, "5B", vec![]), analysis(13000, "", vec![])];
    let playlists = vec![
        Playlist { id: 1, name: "Sets".into(), is_folder: true, ..Default::default() },
        Playlist { id: 2, name: "Warmup".into(), parent_id: 1, track_ids: vec![2, 1], ..Default::default() },
        Playlist { id: 3, name: "Peak".into(), track_ids: vec![3], ..Default::default() },
    ];
    write_usb(usb.path(), &[first, second, third], &analyses, &playlists).unwrap();
    (source, usb)
}

#[test]
fn the_old_writer_stick_reads_from_both_databases() {
    let (_source, usb) = old_writer_stick();
    let stick = Stick::read(usb.path()).unwrap();
    let pdb = stick.device_library.as_ref().unwrap();
    let one = stick.one_library.as_ref().unwrap();

    assert_eq!(pdb.tracks.len(), 3);
    assert_eq!(one.tracks.len(), 3);
    assert_eq!(stick.primary_kind(), Some(LibraryKind::DeviceLibrary));
    for model in [pdb, one] {
        let first = model.track_by_path("/Contents/Alpha/one.wav").unwrap();
        assert_eq!(first.title, "First Light");
        assert_eq!(first.comment.len(), 140);
        assert_eq!(model.artist_name(first.artist_id), "Alpha");
        assert_eq!(model.album_name(first.album_id), "Alpha LP");
        assert_eq!(first.tempo_x100, 12800);
        assert_eq!(first.file_type, 11);
        assert_eq!(model.track(2).unwrap().title, "Zweiter Wind \u{00fc} \u{65e5}\u{672c} \u{0416}");
        assert_eq!(model.track(3).unwrap().artist_id, first.artist_id);
        assert_eq!(model.playlist_tracks(2).iter().map(|track| track.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(model.playlist(2).unwrap().parent_id, 1);
        assert!(model.playlist(1).unwrap().is_folder);
        assert_eq!(model.colors.len(), 8);
        assert_eq!(model.label_name(first.label_id), "Test Label");
    }
    assert_eq!(one.genre_name(one.track(1).unwrap().genre_id), "House");
    assert_eq!(one.key_name(one.track(1).unwrap().key_id), "Db");
    assert_eq!(one.track(1).unwrap().cues.len(), 3);
    assert_eq!(one.track(1).unwrap().cues[1].comment, "drop");
    assert_eq!(one.track(1).unwrap().cues[2].loop_end_ms, Some(1900));
}

#[test]
fn the_old_writer_leaves_genres_and_keys_out_of_export_pdb() {
    let (_source, usb) = old_writer_stick();
    let stick = Stick::read(usb.path()).unwrap();
    let pdb = stick.device_library.as_ref().unwrap();
    let first = pdb.track(1).unwrap();
    assert_ne!((first.genre_id, first.key_id), (0, 0));
    assert!(pdb.genres.is_empty() && pdb.keys.is_empty());
    assert_eq!((pdb.genre_name(first.genre_id), pdb.key_name(first.key_id)), ("", ""));
}

#[test]
fn analysis_is_read_per_track_with_every_section() {
    let (_source, usb) = old_writer_stick();
    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    for track in &model.tracks {
        assert!(stick.audio_path(track).is_file());
        let analysis = stick.read_analysis(track).unwrap();
        assert_eq!(analysis.dat.as_ref().unwrap().path().as_deref(), Some(track.path.as_str()));
        assert_eq!(analysis.beats().len(), 8);
        for file in [&analysis.dat, &analysis.ext] {
            let parsed = file.as_ref().unwrap();
            assert_eq!(parsed.with_path(&track.path), parsed.to_bytes());
        }
    }
    let first = stick.read_analysis(model.track(1).unwrap()).unwrap();
    assert_eq!(first.cues().len(), 3);
    assert!(!first.has_phrases());
    let dat = usb.path().join(model.track(1).unwrap().analyze_path.trim_start_matches('/'));
    assert_eq!(first.dat.unwrap().to_bytes(), std::fs::read(dat).unwrap());
}

#[test]
fn a_stick_with_an_empty_one_library_reads_from_export_pdb() {
    let (_source, usb) = old_writer_stick();
    let full = Stick::read(usb.path()).unwrap().one_library.unwrap();
    let emptied = StickModel { tracks: Vec::new(), playlists: Vec::new(), ..full };
    one_library::write(&usb.path().join("PIONEER/rekordbox/exportLibrary.db"), &emptied).unwrap();

    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.one_library.as_ref().unwrap().tracks.len(), 0);
    assert_eq!(stick.primary_kind(), Some(LibraryKind::DeviceLibrary));
    assert_eq!(stick.track_count(), 3);
    assert_eq!(stick.library().unwrap().playlists.len(), 3);
}

#[test]
fn a_stick_with_only_one_library_reads_from_it() {
    let (_source, usb) = old_writer_stick();
    std::fs::remove_file(usb.path().join("PIONEER/rekordbox/export.pdb")).unwrap();
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.primary_kind(), Some(LibraryKind::OneLibrary));
    assert_eq!(stick.track_count(), 3);
}

#[test]
fn a_hidden_pioneer_folder_is_found() {
    let (_source, usb) = old_writer_stick();
    std::fs::rename(usb.path().join("PIONEER"), usb.path().join(".PIONEER")).unwrap();
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.track_count(), 3);
    assert!(dump_stick(usb.path(), RAW).unwrap().starts_with("stick pioneer_dir=.PIONEER\n"));
}

#[test]
fn a_blank_folder_has_no_library() {
    let blank = TempDir::new().unwrap();
    let stick = Stick::read(blank.path()).unwrap();
    assert!(!stick.has_library());
    assert_eq!(stick.track_count(), 0);
    assert_eq!(dump_stick(blank.path(), RAW).unwrap(), "stick no-library\n");
}

#[test]
fn a_broken_export_pdb_is_an_error_not_an_empty_stick() {
    let (_source, usb) = old_writer_stick();
    std::fs::write(usb.path().join("PIONEER/rekordbox/export.pdb"), b"not a database").unwrap();
    let error = Stick::read(usb.path()).unwrap_err();
    assert!(format!("{error:#}").contains("export.pdb"));
}

#[test]
fn reading_leaves_the_stick_untouched() {
    let (_source, usb) = old_writer_stick();
    let listing = |root: &Path| -> Vec<(String, u64, std::time::SystemTime)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let meta = entry.metadata().unwrap();
                if meta.is_dir() {
                    stack.push(entry.path());
                } else {
                    out.push((entry.path().to_string_lossy().into_owned(), meta.len(), meta.modified().unwrap()));
                }
            }
        }
        out.sort();
        out
    };
    let before = listing(usb.path());
    Stick::read(usb.path()).unwrap();
    dump_stick(usb.path(), RAW).unwrap();
    assert_eq!(listing(usb.path()), before);
}

#[test]
fn two_exports_of_one_library_dump_the_same_and_a_change_shows() {
    let (_first_source, first) = old_writer_stick();
    let (_second_source, second) = old_writer_stick();
    let left = dump_stick(first.path(), RAW).unwrap();
    assert_eq!(diff_dumps(&left, &dump_stick(second.path(), RAW).unwrap()), Vec::<String>::new());
    for prefix in ["pdb header", "pdb table tracks rows=3", "pdb page tracks", "pdb track id=1", "pdb raw tracks", "onelib track id=1", "onelib schema table content", "onelib row cue", "anlz USBANLZ/", "audio Contents/Alpha/one.wav"] {
        assert!(left.lines().any(|line| line.starts_with(prefix)), "no line starts with {prefix:?}");
    }

    let pdb_path = second.path().join("PIONEER/rekordbox/export.pdb");
    let mut model = device_library::read(&std::fs::read(&pdb_path).unwrap()).unwrap();
    model.tracks[0].rating = 4;
    std::fs::write(&pdb_path, device_library::build(&model).unwrap()).unwrap();
    let changed = diff_dumps(&left, &dump_stick(second.path(), RAW).unwrap());
    assert!(changed.iter().any(|line| line.starts_with("+pdb track id=1 ") && line.contains(" rating=4 ")), "{changed:#?}");
    assert!(changed.iter().all(|line| !line.contains("onelib ") && !line.contains("anlz ")), "{changed:#?}");
}

#[test]
fn player_files_are_listed_as_not_compared() {
    let (_source, usb) = old_writer_stick();
    std::fs::write(usb.path().join("PIONEER/rekordbox/RBFLTR.DAT"), b"player").unwrap();
    std::fs::write(usb.path().join("PIONEER/._hidden"), b"mac").unwrap();
    let dump = dump_stick(usb.path(), RAW).unwrap();
    assert!(dump.contains("not-compared rekordbox/RBFLTR.DAT"));
    assert!(dump.contains("not-compared ._hidden"));
    assert!(!dump.contains("file rekordbox/RBFLTR.DAT"));
}
